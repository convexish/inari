use std::collections::BTreeMap;

use super::super::table::plain;
use super::*;

/// Every deal the app offers: each of the sixteen Artifacts,
/// Echoes, Figures and Cities subsets, with and without Unseen.
const BASE_MODES: [n::Expansions; 16] = [
    n::Expansions::BASE,
    n::Expansions::FIGURES,
    n::Expansions::CITIES,
    n::Expansions::FIGURES.union(n::Expansions::CITIES),
    n::Expansions::ECHOES,
    n::Expansions::ECHOES.union(n::Expansions::FIGURES),
    n::Expansions::ECHOES.union(n::Expansions::CITIES),
    n::Expansions::ECHOES
        .union(n::Expansions::FIGURES)
        .union(n::Expansions::CITIES),
    n::Expansions::ARTIFACTS,
    n::Expansions::ARTIFACTS.union(n::Expansions::FIGURES),
    n::Expansions::ARTIFACTS.union(n::Expansions::CITIES),
    n::Expansions::ARTIFACTS
        .union(n::Expansions::FIGURES)
        .union(n::Expansions::CITIES),
    n::Expansions::ARTIFACTS.union(n::Expansions::ECHOES),
    n::Expansions::ARTIFACTS
        .union(n::Expansions::ECHOES)
        .union(n::Expansions::FIGURES),
    n::Expansions::ARTIFACTS
        .union(n::Expansions::ECHOES)
        .union(n::Expansions::CITIES),
    n::Expansions::ARTIFACTS
        .union(n::Expansions::ECHOES)
        .union(n::Expansions::FIGURES)
        .union(n::Expansions::CITIES),
];

const MODES: [n::Expansions; 32] = {
    let mut modes = [n::Expansions::BASE; 32];
    let mut i = 0;
    while i < 16 {
        modes[i] = BASE_MODES[i];
        modes[16 + i] = BASE_MODES[i].union(n::Expansions::UNSEEN);
        i += 1;
    }
    modes
};

fn dealt(expansions: n::Expansions, seed: u64) -> Match {
    let mut m = Match::new(expansions, seed).unwrap();
    m.apply(Step::Choice(0)).unwrap();
    m.apply(Step::Choice(0)).unwrap();
    assert!(matches!(m.prompt(), Prompt::Turn { .. }));
    m
}

fn codes(game: &Game, cards: &[CardId]) -> Vec<String> {
    cards
        .iter()
        .map(|&c| game.position().card(c).code.into())
        .collect()
}

fn face_codes(cards: &[FaceId]) -> Vec<String> {
    cards.iter().map(|&c| face(c).code.clone()).collect()
}

fn check_sorted_pile(game: &Game, cards: &[CardId], shown: &[FaceId]) {
    let mut expected = cards.to_vec();
    expected.sort_by_key(|&c| {
        let card = game.position().card(c);
        (card.value, Set::from(card.set), stack_index(card.color))
    });
    assert_eq!(face_codes(shown), codes(game, &expected));
}

#[test]
fn museum_artifacts_are_named_and_clickable_meld_options() {
    let expansions = n::Expansions::from_bits(0b11111).unwrap();
    let mut museums = 0;
    for seed in 1..=6 {
        let mut m = Match::new(expansions, seed).unwrap();
        let mut rng = n::Rng::new(seed);
        for _ in 0..600 {
            let Some(p) = m.to_move() else { break };
            if let Prompt::Turn { actions, .. } = m.prompt() {
                let v = view(&m, index(p), false, false);
                let options = &v.decision.as_ref().unwrap().options;
                let offered = |card| {
                    let face = face_of(m.game(), card);
                    options.iter().any(|o| o.card == Some(face))
                };
                let pos = m.game().position();
                for &card in pos.cards(Zone::Museum(p)) {
                    assert!(actions.contains(&Action::Meld { card }));
                    assert!(offered(card), "seed {seed}: Museum meld without its card");
                    museums += 1;
                }
            }
            let steps = option_steps(&m);
            let step = steps[(rng.next_u64() % steps.len() as u64) as usize];
            m.apply(step).unwrap();
        }
    }
    assert!(museums > 0, "the seeds reach Museum melds");
}

#[test]
fn seeded_snapshots_match_native_piles_options_counts_and_clock() {
    let mut completed = [0; MODES.len()];
    let mut choices = 0;
    let mut legal_achievements = 0;
    for (mode, expansions) in MODES.into_iter().enumerate() {
        for seed in 1..=2 {
            let mut m = Match::new(expansions, seed).unwrap();
            for step in 0..600 {
                let pos = m.game().position();
                for viewer in 0..2 {
                    let v = view(&m, viewer, step % 2 == 0, false);
                    assert_eq!(v.figures, expansions.figures());
                    assert_eq!(v.cities, expansions.cities());
                    assert_eq!(v.echoes, expansions.echoes());
                    assert_eq!(v.unseen, expansions.unseen());
                    assert_eq!(v.artifacts, expansions.artifacts());
                    assert_eq!(v.supply.museums_available, pos.available_museums());
                    assert_eq!(v.supply.museums_junked, pos.junked_museums());
                    assert_eq!(v.private_owner, viewer);
                    assert_eq!(v.current_player, index(pos.turn_owner()));
                    assert_eq!(v.achievements_needed, 6 + expansions.count());
                    assert_eq!(
                        v.decision.as_ref().map_or(0, |d| d.options.len()),
                        option_steps(&m).len()
                    );
                    if let Some(d) = &v.decision {
                        assert_eq!(Some(player(d.player)), m.to_move());
                        assert!(
                            d.options
                                .iter()
                                .all(|o| !plain(&o.segs, viewer, false).is_empty())
                        );
                    }
                    match m.prompt() {
                        Prompt::Turn { .. } => {
                            assert_eq!(v.action, usize::from(pos.regular_actions_started()) + 1)
                        }
                        Prompt::Decision(_) | Prompt::Over(_)
                            if pos.regular_actions_started() > 0 =>
                        {
                            assert_eq!(v.action, usize::from(pos.regular_actions_started()))
                        }
                        _ => {}
                    }
                    assert_eq!(v.actions, if v.turn == 1 { 1 } else { 2 });
                    for p in Player::ALL {
                        let pv = &v.players[index(p)];
                        for (i, c) in COLORS.into_iter().enumerate() {
                            let st = &pv.stacks[i];
                            assert_eq!(
                                face_codes(&st.cards),
                                codes(m.game(), pos.cards(Zone::Board(p, c)))
                            );
                            assert_eq!(st.splay, pos.splay(p, c));
                            assert!(st.cards.iter().all(|&id| face(id).color == c));
                        }
                        check_sorted_pile(m.game(), pos.cards(Zone::Hand(p)), &pv.hand);
                        check_sorted_pile(m.game(), pos.cards(Zone::Score(p)), &pv.score);
                        check_sorted_pile(m.game(), pos.cards(Zone::Forecast(p)), &pv.forecast);
                        assert_eq!(pv.forecast_limit, pos.forecast_limit(p));
                        let cards: Vec<_> = pv
                            .achievements
                            .iter()
                            .filter_map(|a| match a {
                                Held::Card(c) => Some(*c),
                                _ => None,
                            })
                            .collect();
                        assert_eq!(
                            face_codes(&cards),
                            codes(m.game(), pos.cards(Zone::Achievements(p)))
                        );
                        let specials: Vec<_> = pv
                            .achievements
                            .iter()
                            .filter_map(|a| match a {
                                Held::Special(s) => Some(*s),
                                _ => None,
                            })
                            .collect();
                        assert_eq!(
                            specials,
                            pos.specials()
                                .filter(|&s| pos.special_location(s) == SpecialLocation::Owned(p))
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            pv.decrees,
                            Decree::ALL
                                .into_iter()
                                .filter(|&d| pos.decree_location(d) == DecreeLocation::Owned(p))
                                .map(|d| (decree(d).to_owned(), d.color()))
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(pv.achievement_count, pos.effective_achievement_count(p));
                        assert_eq!(pv.score_total, pos.score(p));
                        assert_eq!(pv.bonus_score, pos.bonus_score(p));
                        assert_eq!(pv.icons, ICONS.map(|i| pos.icon_count(p, i)));
                        assert_eq!(pv.safe.len(), pos.cards(Zone::Safe(p)).len());
                        assert_eq!(
                            pv.display,
                            pos.cards(Zone::Display(p))
                                .first()
                                .map(|&c| face_of(m.game(), c))
                        );
                        assert_eq!(
                            pv.museums.iter().map(|(_, f)| *f).collect::<Vec<_>>(),
                            pos.cards(Zone::Museum(p))
                                .iter()
                                .map(|&c| face_of(m.game(), c))
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            pv.achievements
                                .iter()
                                .filter(|h| matches!(h, Held::Museum(_)))
                                .count(),
                            pos.claimed_museums(p)
                        );
                    }
                    assert_eq!(
                        face_codes(&v.supply.junk),
                        codes(m.game(), pos.cards(Zone::Junk))
                    );
                    let available: BTreeSet<_> = v
                        .supply
                        .specials
                        .iter()
                        .flatten()
                        .map(|&s| s as usize)
                        .collect();
                    let expected: BTreeSet<_> = pos
                        .specials()
                        .filter(|&s| pos.special_location(s) == SpecialLocation::Available)
                        .map(|s| s as usize)
                        .collect();
                    assert_eq!(available, expected);
                    let sets: Vec<_> = n::CardSet::ALL
                        .into_iter()
                        .filter(|&s| expansions.includes(s))
                        .collect();
                    assert_eq!(v.supply.decks.len(), sets.len());
                    for (row, &s) in v.supply.decks.iter().zip(&sets) {
                        assert_eq!(row.set, Set::from(s));
                        for age in 0..=11 {
                            assert_eq!(
                                row.counts[age],
                                pos.cards(Zone::Deck(n::Deck {
                                    set: s,
                                    age: age as u8
                                }))
                                .len()
                            );
                        }
                        if !pos.age_zero_introduced() {
                            assert_eq!(row.counts[0], 0);
                        }
                    }
                    let mut expected: Vec<_> = pos
                        .cards(Zone::AvailableAchievements)
                        .iter()
                        .map(|&c| {
                            let printed = pos.card(c).origin_deck();
                            (printed.age, Set::from(printed.set))
                        })
                        .collect();
                    expected.sort();
                    assert_eq!(
                        v.supply
                            .achievements
                            .iter()
                            .map(|a| (a.age, a.set))
                            .collect::<Vec<_>>(),
                        expected
                    );
                    for a in &v.supply.achievements {
                        let legal = m.to_move() == Some(player(viewer))
                            && achieve_step(&m, a.age, a.set).is_some();
                        assert_eq!(a.legal, legal);
                        legal_achievements += usize::from(legal);
                    }
                    if matches!(m.prompt(), Prompt::Over(_)) {
                        assert!(v.over.is_some());
                    } else {
                        assert!(v.over.is_none());
                    }
                }
                let Some(next) = m.random_step(seed * 1000 + step) else {
                    completed[mode] += 1;
                    break;
                };
                choices += usize::from(matches!(next, Step::Choice(_)));
                m.apply(next).unwrap();
            }
        }
    }
    assert!(
        completed.into_iter().all(|count| count > 0),
        "bounded games must reach terminal prompts"
    );
    assert!(choices > 50);
    assert!(legal_achievements > 0);
}

// Independent physical-location oracle. In the log, ordinary reveals
// authorize only their event; menus may also name this action's reveals
// and Search's active return packet (checked below).
fn locations(game: &Game) -> BTreeMap<CardId, Zone> {
    let mut zones = vec![
        Zone::Junk,
        Zone::AvailableAchievements,
        Zone::AgeZeroReserve,
    ];
    for p in Player::ALL {
        zones.extend([
            Zone::Hand(p),
            Zone::Score(p),
            Zone::Forecast(p),
            Zone::Safe(p),
            Zone::Display(p),
            Zone::Museum(p),
            Zone::Achievements(p),
        ]);
        zones.extend(n::Color::ALL.map(|c| Zone::Board(p, c)));
    }
    for s in n::CardSet::ALL {
        zones.extend((0..=11).map(|age| Zone::Deck(n::Deck { set: s, age })));
    }
    zones
        .into_iter()
        .flat_map(|z| game.position().cards(z).iter().map(move |&c| (c, z)))
        .collect()
}

/// Zones whose fronts `viewer` may see: public ones, their own private
/// piles, and the other player's while the engine grants a look at them
/// (a Hitchhiking controller inspecting the executor's hand and score).
fn public_to(game: &Game, z: Zone, viewer: usize) -> bool {
    matches!(z, Zone::Board(..) | Zone::Display(_) | Zone::Museum(_))
        || (matches!(
            z,
            Zone::Hand(_) | Zone::Score(_) | Zone::Forecast(_) | Zone::Junk
        ) && game.position().can_view(z, player(viewer)))
}

struct MovementVisibility {
    card: CardId,
    sees_zone: [bool; 2],
    publicly_known_at_destination: bool,
}

/// A viewing grant can expire before the next decision. Replay a frame at
/// a time to check physical visibility while each movement is taking place.
fn movement_visibility(m: &Match) -> Vec<MovementVisibility> {
    let record = m.record();
    let mut game = Game::new_game(record.seed, record.expansions)
        .unwrap()
        .with_tracking()
        .unwrap()
        .with_narration()
        .with_subgames(record.subgames);
    let mut movements = Vec::new();
    let mut events = Vec::new();
    for step in std::iter::once(None).chain(record.steps.iter().copied().map(Some)) {
        match step {
            Some(Step::Action(action)) => game.take_action(action, n::ActionKind::Regular).unwrap(),
            Some(Step::Choice(index)) => game.choose(index).unwrap(),
            None => {}
        }
        loop {
            let status = game.advance(1, &mut events).unwrap();
            for event in events.drain(..) {
                let mut moved = |card, source, destination| {
                    let sees_zone = [0, 1].map(|viewer| {
                        public_to(&game, source, viewer) || public_to(&game, destination, viewer)
                    });
                    let publicly_known_at_destination = game
                        .public_tracker()
                        .unwrap()
                        .belief(destination)
                        .iter()
                        .any(|slot| slot.known() == Some(card));
                    movements.push(MovementVisibility {
                        card,
                        sees_zone,
                        publicly_known_at_destination,
                    });
                };
                match event {
                    Event::Moved(mv) => moved(mv.card, mv.source, mv.destination),
                    Event::Exchanged { movements, .. } => {
                        for mv in movements {
                            moved(mv.card, mv.source, mv.destination);
                        }
                    }
                    _ => {}
                }
            }
            if !matches!(status, n::Status::WorkLimit) {
                break;
            }
        }
    }
    movements
}

fn assert_private_log(
    m: &Match,
    initial: &BTreeMap<CardId, Zone>,
    viewer: usize,
    log: &[Entry],
    movements: &[MovementVisibility],
) {
    let mut movements = movements.iter();
    let mut physical = initial.clone();
    let mut events = m.events().iter().peekable();
    let mut allowed = Vec::<BTreeSet<FaceId>>::new();
    let mut shown = BTreeMap::<CardId, Zone>::new();
    for tag in 0..=m.record().steps.len() {
        if let Some(Step::Action(a)) = tag.checked_sub(1).and_then(|i| m.record().steps.get(i)) {
            match a {
                Action::Dogma { card, .. } => {
                    assert!(matches!(physical[card], Zone::Board(..)));
                    allowed.push(BTreeSet::from([face_of(m.game(), *card)]));
                }
                Action::Decree => allowed.push(BTreeSet::new()),
                _ => {}
            }
        }
        while events.peek().is_some_and(|(step, _)| *step == tag) {
            let (_, event) = events.next().unwrap();
            // A turn start is a header, and a regular Dogma's line comes
            // from its input above.
            if matches!(
                event,
                Event::TurnBegan { .. }
                    | Event::ExecutionEnded { .. }
                    | Event::DogmaStarted {
                        kind: n::DogmaKind::Action,
                        ..
                    }
            ) {
                continue;
            }
            let mut visible = BTreeSet::new();
            let mut moved = |card: CardId, src: Zone, dst: Zone| {
                let seen = movements.next().expect("replayed movement");
                assert_eq!(seen.card, card);
                assert_eq!(physical.insert(card, dst), Some(src));
                // Follow a disclosed identity through a hidden move only while
                // the public tracker can still identify it at the destination.
                let followed =
                    shown.remove(&card) == Some(src) && seen.publicly_known_at_destination;
                if seen.sees_zone[viewer] || followed {
                    visible.insert(face_of(m.game(), card));
                }
                if followed || seen.sees_zone == [true, true] {
                    shown.insert(card, dst);
                }
            };
            match event {
                Event::Moved(mv) => moved(mv.card, mv.source, mv.destination),
                Event::Exchanged { movements, .. } => {
                    for mv in movements {
                        moved(mv.card, mv.source, mv.destination);
                    }
                }
                Event::Revealed { card, source, .. } => {
                    assert_eq!(physical[card], *source);
                    visible.insert(face_of(m.game(), *card));
                    shown.insert(*card, *source);
                }
                Event::FigureSearch {
                    deck,
                    name,
                    found: Some(card),
                    ..
                } => {
                    let source = Zone::Deck(*deck);
                    assert_eq!(physical[card], source);
                    assert_eq!(
                        name.expect("a search hit names a Figure").code,
                        m.game().position().card(*card).code
                    );
                    shown.insert(*card, source);
                }
                Event::Shuffled { deck, .. } => {
                    shown.retain(|_, source| *source != Zone::Deck(*deck));
                }
                // An endorsed Dogma's card is a public top card, a free
                // Dogma's a public top or displayed card, and a karma's
                // Figure a public top card.
                Event::Endorsed { card, .. } | Event::DogmaStarted { card, .. } => {
                    assert!(public_to(m.game(), physical[card], viewer));
                    visible.insert(face_of(m.game(), *card));
                }
                // Executing a card reads its text to everyone, wherever
                // it lies; the card whose effect or karma asked for it
                // is itself executing (a public top, display or
                // disclosed card).
                Event::ExecutionStarted { card, source, .. } => {
                    visible.insert(face_of(m.game(), *card));
                    if let Some(source) = source {
                        visible.insert(face_of(m.game(), *source));
                    }
                }
                // The interrupted operation's card shows from where it is.
                Event::KarmaTriggered { card, would, .. } => {
                    assert!(public_to(m.game(), physical[card], viewer));
                    visible.insert(face_of(m.game(), *card));
                    if let Some(
                        n::Operation::Move { target, .. } | n::Operation::Claim { target, .. },
                    ) = would
                        && public_to(m.game(), physical[&target.card], viewer)
                    {
                        visible.insert(face_of(m.game(), target.card));
                    }
                    if let Some(n::Operation::Dogma { card, .. }) = would {
                        assert!(public_to(m.game(), physical[card], viewer));
                        visible.insert(face_of(m.game(), *card));
                    }
                }
                Event::AgeZeroIntroduced { .. } => {
                    for z in physical.values_mut() {
                        if *z == Zone::AgeZeroReserve {
                            *z = Zone::Deck(n::Deck::base(0));
                        }
                    }
                }
                _ => {}
            }
            allowed.push(visible);
        }
    }
    assert!(movements.next().is_none());
    assert_eq!(physical, locations(m.game()));
    let mut allowed = allowed.iter();
    for entry in log {
        if let Entry::Line(_, segs, count) = entry {
            for _ in 0..*count {
                let permission = allowed.next().expect("one permission per log line");
                for seg in segs {
                    if let Seg::Card(id) = seg {
                        assert!(
                            permission.contains(id),
                            "{} not physically visible at line: {}",
                            face(*id).code,
                            plain(segs, viewer, false)
                        );
                    }
                }
            }
        }
    }
    assert!(allowed.next().is_none(), "every fact must have a line");
}

#[test]
fn seeded_logs_and_menus_only_expose_visible_packet_or_retained_source_faces() {
    let mut backs = 0;
    for expansions in MODES {
        for seed in [10] {
            let mut m = Match::new(expansions, seed).unwrap();
            let initial = locations(m.game());
            for step in 0..400 {
                for viewer in 0..2 {
                    let v = view(&m, viewer, true, false);
                    let mut visible: BTreeSet<_> = locations(m.game())
                        .into_iter()
                        .filter(|(_, z)| public_to(m.game(), *z, viewer))
                        .map(|(c, _)| face_of(m.game(), c))
                        .collect();
                    if let Prompt::Decision(native) = m.prompt() {
                        let action_step = m
                            .record()
                            .steps
                            .iter()
                            .rposition(|step| matches!(step, Step::Action(_)))
                            .map_or(0, |i| i + 1);
                        // Both players saw this action's reveals.
                        for (step, event) in m.events() {
                            if let Event::Revealed { card, .. } = event
                                && *step >= action_step
                            {
                                visible.insert(face_of(m.game(), *card));
                            }
                        }
                        for option in &native.options {
                            if let Alternative::Move {
                                verb: Verb::Return,
                                target:
                                    TargetDescriptor::Known {
                                        card,
                                        source: source @ Zone::Deck(_),
                                    },
                            } = option
                            {
                                assert!(
                                    m.events().iter().any(|(step, event)| {
                                        *step >= action_step
                                            && matches!(event,
                                            Event::Revealed { card: c, source: z, .. }
                                                if c == card && z == source)
                                    }),
                                    "Search returns must have been revealed during this action"
                                );
                                visible.insert(face_of(m.game(), *card));
                            }
                        }
                    }
                    if let Some(d) = v.decision {
                        let retained_source = match m.prompt() {
                            Prompt::Decision(native) => native
                                .source_card(player(viewer))
                                .map(|card| face_of(m.game(), card)),
                            _ => None,
                        };
                        for seg in &d.prompt {
                            if let Seg::Card(id) = seg {
                                assert!(visible.contains(id) || Some(*id) == retained_source);
                            }
                        }
                        for option in d.options {
                            if let Some(id) = option.card {
                                assert!(visible.contains(&id));
                            }
                            for seg in option.segs {
                                if let Seg::Card(id) = seg {
                                    assert!(visible.contains(&id));
                                }
                                if let Seg::Back(..) = seg {
                                    backs += 1;
                                }
                            }
                        }
                    }
                }
                let Some(next) = m.random_step(seed * 2000 + step) else {
                    break;
                };
                m.apply(next).unwrap();
            }
            // The log is built from the event history alone, so the final
            // log covers every line each earlier log showed.
            let movements = movement_visibility(&m);
            for viewer in 0..2 {
                assert_private_log(
                    &m,
                    &initial,
                    viewer,
                    &log_entries(&m, viewer, true),
                    &movements,
                );
            }
        }
    }
    assert!(backs > 100);
}

#[test]
fn opponent_draw_staying_in_hand_is_only_a_back_even_when_menu_names_it() {
    let mut m = dealt(n::Expansions::BASE, 17);
    let drawing = m.game().position().turn_owner();
    let viewer = index(drawing.opponent());
    let before = m.events().len();
    m.apply(Step::Action(Action::Draw)).unwrap();
    let Event::Moved(draw) = &m.events()[before].1 else {
        panic!("expected draw")
    };
    let id = face_of(m.game(), draw.card);
    assert_eq!(draw.destination, Zone::Hand(drawing));
    assert!(
        m.game()
            .position()
            .cards(Zone::Hand(drawing))
            .contains(&draw.card)
    );
    let v = view(&m, viewer, false, false);
    for entry in &v.log {
        if let Entry::Line(_, segs, _) = entry {
            assert!(!segs.contains(&Seg::Card(id)));
        }
    }
    assert_eq!(
        plain(
            &sentence(m.game(), &m.events()[before].1, viewer),
            viewer,
            false
        ),
        format!("AI draws [Base {}]", face(id).age)
    );
    assert_eq!(
        plain(
            &sentence(m.game(), &m.events()[before].1, index(drawing)),
            index(drawing),
            false
        ),
        format!(
            "You draw {} [{}]",
            title_case(&face(id).title),
            face(id).age
        )
    );
    // Both a Known descriptor and a native self-execute reference must be
    // re-filtered; neither is authority to expose an opponent's hand.
    let options = Options::new(m.game(), viewer, drawing);
    for alternative in [
        Alternative::Move {
            verb: Verb::Meld,
            target: TargetDescriptor::Known {
                card: draw.card,
                source: draw.destination,
            },
        },
        Alternative::SelfExecute(draw.card),
    ] {
        let o = options.alternative(&alternative);
        assert_eq!(o.card, None);
        assert!(o.segs.contains(&back(id)));
    }
}

#[test]
fn oars_impossible_transfer_lists_the_revealed_hand_in_the_app_log() {
    let cards = ["B003", "B001", "B006", "B004"].map(|code| n::Card::from_code(code).unwrap());
    let mut game = Game::new(
        n::Position::new(
            cards.to_vec(),
            &[
                (CardId(0), Zone::Board(Player::One, n::Color::Red)),
                (CardId(1), Zone::Hand(Player::Two)),
                (CardId(2), Zone::Hand(Player::Two)),
                (CardId(3), Zone::Deck(n::Deck::base(1))),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::ECHOES,
    )
    .unwrap();
    game.take_action(
        Action::Dogma {
            card: CardId(0),
            auspice: false,
        },
        n::ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(matches!(
        game.advance(4096, &mut events).unwrap(),
        n::Status::Complete(_)
    ));
    let lines: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, Event::Revealed { .. }))
        .map(|event| sentence(&game, event, 0))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        plain(&lines[0], 0, false),
        "AI reveals Archery [1] from their hand"
    );
    assert_eq!(
        plain(&lines[1], 0, false),
        "AI reveals Masonry [1] from their hand"
    );
    assert!(lines[0].contains(&Seg::Card(face_of(&game, CardId(1)))));
    assert!(lines[1].contains(&Seg::Card(face_of(&game, CardId(2)))));
}

fn activate_maze(other_card: &str) -> Game {
    let cards = ["U002", "B001", "B003", other_card].map(|code| n::Card::from_code(code).unwrap());
    let mut game = Game::new(
        n::Position::new(
            cards.to_vec(),
            &[
                (CardId(0), Zone::Board(Player::One, n::Color::Red)),
                (CardId(1), Zone::Hand(Player::One)),
                (CardId(2), Zone::Hand(Player::Two)),
                (CardId(3), Zone::Hand(Player::Two)),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::UNSEEN,
    )
    .unwrap();
    game.take_action(
        Action::Dogma {
            card: CardId(0),
            auspice: false,
        },
        n::ActionKind::Regular,
    )
    .unwrap();
    game
}

#[test]
fn maze_names_the_scored_card_its_color_proof_revealed() {
    // Maze demands a red card from the opponent's hand; Oars is scored only
    // after a reveal proves its color, while Agriculture stays hidden.
    let mut game = activate_maze("B004");
    let mut events = Vec::new();
    assert!(matches!(
        game.advance(4096, &mut events).unwrap(),
        n::Status::Complete(_)
    ));
    let lines: Vec<_> = events
        .iter()
        .map(|event| plain(&sentence(&game, event, 0), 0, false))
        .collect();
    assert_eq!(
        lines,
        [
            "AI reveals Archery [1] from your hand",
            "AI reveals Oars [1] from their hand",
            "AI scores Oars [1]",
        ]
    );
    let scored = events
        .iter()
        .find(|event| matches!(event, Event::Moved(m) if m.verb == Some(Verb::Score)))
        .expect("Oars is scored");
    let Event::Moved(movement) = scored else {
        unreachable!()
    };
    assert_eq!(movement.card, CardId(2));
    assert!(movement.public);
    for (viewer, expected) in [(0, "AI scores Oars [1]"), (1, "You score Oars [1]")] {
        let line = sentence(&game, scored, viewer);
        assert_eq!(plain(&line, viewer, false), expected);
        assert!(line.contains(&Seg::Card(face_of(&game, CardId(2)))));
    }
}

#[test]
fn maze_logs_each_scored_card_when_the_target_chooses_among_matches() {
    let mut game = activate_maze("B002");
    let mut events = Vec::new();
    let n::Status::Decision(menu) = game.advance(4096, &mut events).unwrap() else {
        panic!("target chooses which matching red card to score");
    };
    assert_eq!(menu.player, Player::Two);
    let pick = menu
        .options
        .iter()
        .position(|option| {
            matches!(
                option,
                Alternative::Move {
                    verb: Verb::Score,
                    target: TargetDescriptor::Known {
                        card: CardId(3),
                        ..
                    },
                }
            )
        })
        .expect("Metalworking is an option");
    game.choose(pick).unwrap();
    assert!(matches!(
        game.advance(4096, &mut events).unwrap(),
        n::Status::Complete(_)
    ));
    let lines: Vec<_> = events
        .iter()
        .map(|event| plain(&sentence(&game, event, 0), 0, false))
        .collect();
    assert_eq!(
        lines,
        vec![
            "AI reveals Archery [1] from your hand",
            "AI reveals Metalworking [1] from their hand",
            "AI scores Metalworking [1]",
        ]
    );
}

#[test]
fn propaganda_and_ninja_log_the_demanded_color() {
    for (code, color) in [("U024", n::Color::Purple), ("U036", n::Color::Red)] {
        let cards = vec![
            n::Card::from_code(code).unwrap(),
            n::Card::from_code("B001").unwrap(),
        ];
        let mut game = Game::new(
            n::Position::new(
                cards,
                &[
                    (CardId(0), Zone::Board(Player::One, color)),
                    (CardId(1), Zone::Hand(Player::Two)),
                ],
                Player::One,
            )
            .unwrap(),
            n::Expansions::UNSEEN,
        )
        .unwrap();
        game.take_action(
            Action::Dogma {
                card: CardId(0),
                auspice: false,
            },
            n::ActionKind::Regular,
        )
        .unwrap();
        let n::Status::Decision(menu) = game.advance(4096, &mut Vec::new()).unwrap() else {
            panic!("{code}: expected the demanded color");
        };
        assert_eq!(menu.player, Player::One);
        let red = menu
            .options
            .iter()
            .position(|o| *o == Alternative::Colors(1 << n::Color::Red as u8))
            .unwrap();
        let announced = menu.announcement(red).expect("a color choice is public");
        for (viewer, expected) in [(0, "You choose red"), (1, "AI chooses red")] {
            assert_eq!(
                plain(&sentence(&game, &announced, viewer), viewer, false),
                expected
            );
        }
    }
}

#[test]
fn blackmail_menus_name_the_cards_this_action_revealed() {
    let cards = vec![
        n::Card::from_code("U040").unwrap(),
        n::Card::from_code("B001").unwrap(),
        n::Card::from_code("B002").unwrap(),
        n::Card::from_code("B003").unwrap(),
    ];
    let mut game = Game::new(
        n::Position::new(
            cards,
            &[
                (CardId(0), Zone::Board(Player::One, n::Color::Green)),
                (CardId(1), Zone::Hand(Player::Two)),
                (CardId(2), Zone::Hand(Player::Two)),
                (CardId(3), Zone::Score(Player::Two)),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::UNSEEN,
    )
    .unwrap();
    game.take_action(
        Action::Dogma {
            card: CardId(0),
            auspice: false,
        },
        n::ActionKind::Regular,
    )
    .unwrap();
    let named = |game: &Game, menu: &n::Decision, events: &[Event], viewer| {
        effect_decision(game, menu, viewer, &revealed_cards(events.iter()))
            .options
            .iter()
            .map(|o| o.card)
            .collect::<Option<BTreeSet<_>>>()
    };
    let [first, second, scored] = [1, 2, 3].map(|c| face_of(&game, CardId(c)));
    let mut events = Vec::new();
    let n::Status::Decision(meld) = game.advance(4096, &mut events).unwrap() else {
        panic!("Blackmail asks which revealed card to meld");
    };
    assert_eq!(meld.player, Player::One);
    // The hand is not the demander's to look at; only the reveal names it.
    let unrevealed = effect_decision(&game, &meld, 0, &BTreeSet::new());
    assert!(unrevealed.options.iter().all(|o| o.card.is_none()));
    for viewer in 0..2 {
        assert_eq!(
            named(&game, &meld, &events, viewer),
            Some(BTreeSet::from([first, second]))
        );
    }
    game.choose(0).unwrap();
    let n::Status::Decision(execute) = game.advance(4096, &mut events).unwrap() else {
        panic!("Blackmail asks which revealed card to self-execute");
    };
    assert_eq!(execute.player, Player::One);
    for viewer in 0..2 {
        assert_eq!(
            named(&game, &execute, &events, viewer),
            Some(BTreeSet::from([first, second, scored]))
        );
    }
}

#[test]
fn blackmail_names_revealed_cards_after_bear_grylls_junks_them() {
    let mut cards = vec![
        n::Card::from_code("U040").unwrap(),
        n::Card::from_code("F110").unwrap(),
        n::Card::from_code("B001").unwrap(),
    ];
    cards.extend(["Z001", "Z002"].map(|code| n::Card::from_code(code).unwrap()));
    let mut game = Game::new(
        n::Position::new(
            cards,
            &[
                (CardId(0), Zone::Board(Player::One, n::Color::Green)),
                (CardId(1), Zone::Hand(Player::Two)),
                (CardId(2), Zone::Hand(Player::Two)),
                (CardId(3), Zone::AgeZeroReserve),
                (CardId(4), Zone::AgeZeroReserve),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::FIGURES.union(n::Expansions::UNSEEN),
    )
    .unwrap();
    game.take_action(
        Action::Dogma {
            card: CardId(0),
            auspice: false,
        },
        n::ActionKind::Regular,
    )
    .unwrap();
    let mut events = Vec::new();
    let n::Status::Decision(meld) = game.advance(4096, &mut events).unwrap() else {
        panic!("Blackmail asks which revealed card to meld");
    };
    let bear = meld
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Select {
                    target: TargetDescriptor::Known {
                        card: CardId(1),
                        ..
                    },
                }
            )
        })
        .unwrap();
    game.choose(bear).unwrap();
    // Bear Grylls junks every card outside the decks, in any order.
    let execute = loop {
        let n::Status::Decision(menu) = game.advance(4096, &mut events).unwrap() else {
            panic!("Blackmail asks which revealed card to self-execute");
        };
        if menu.player == Player::One {
            break menu;
        }
        game.choose(0).unwrap();
    };
    let revealed = [CardId(1), CardId(2)];
    for card in revealed {
        assert!(game.position().cards(Zone::Junk).contains(&card));
    }
    for viewer in 0..2 {
        let shown = effect_decision(&game, &execute, viewer, &revealed_cards(events.iter()));
        assert_eq!(
            shown
                .options
                .iter()
                .map(|o| o.card)
                .collect::<Option<BTreeSet<_>>>(),
            Some(revealed.map(|card| face_of(&game, card)).into())
        );
    }
}

#[test]
fn forecast_promotion_names_owner_choices_and_hides_them_from_other_seat() {
    let cards = vec![
        n::Card::from_code("B020").unwrap(),
        n::Card::from_code("E013").unwrap(),
        n::Card::from_code("E001").unwrap(),
        n::Card::from_code("E002").unwrap(),
        n::Card::from_code("B003").unwrap(),
    ];
    let mut game = Game::new(
        n::Position::new(
            cards,
            &[
                (CardId(0), Zone::Hand(Player::One)),
                (CardId(1), Zone::Forecast(Player::One)),
                (CardId(2), Zone::Forecast(Player::One)),
                (CardId(3), Zone::Forecast(Player::Two)),
                (CardId(4), Zone::Hand(Player::One)),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::ECHOES,
    )
    .unwrap();
    let own = player_view(&game, Player::One, 0);
    assert_eq!(own.forecast.len(), 2);
    assert_eq!(own.forecast_limit, 5);
    assert_eq!(player_view(&game, Player::Two, 0).forecast.len(), 1);
    // An opponent's native Known descriptor still exposes only the back.
    let opposing = Options::new(&game, 0, Player::Two).target(
        "Select ",
        &TargetDescriptor::Known {
            card: CardId(3),
            source: Zone::Forecast(Player::Two),
        },
    );
    assert_eq!(opposing.card, None);
    assert!(opposing.segs.contains(&Seg::Back(Set::Echoes, 1)));

    game.take_action(Action::Meld { card: CardId(0) }, n::ActionKind::Regular)
        .unwrap();
    let mut events = Vec::new();
    let n::Status::Decision(menu) = game.advance(4096, &mut events).unwrap() else {
        panic!("choose one of the two eligible Forecast cards");
    };
    let own = effect_decision(&game, &menu, 0, &BTreeSet::new());
    assert_eq!(own.options.len(), 2);
    assert_eq!(
        own.options
            .iter()
            .filter_map(|o| o.card)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([face_of(&game, CardId(1)), face_of(&game, CardId(2))])
    );
    let other = effect_decision(&game, &menu, 1, &BTreeSet::new());
    assert!(
        other
            .options
            .iter()
            .all(|o| o.card.is_none() && o.segs.contains(&Seg::Back(Set::Echoes, 1)))
    );

    let bell = menu
        .options
        .iter()
        .position(|o| {
            matches!(
                o,
                Alternative::Move {
                    verb: Verb::Meld,
                    target: TargetDescriptor::Known {
                        card: CardId(1),
                        ..
                    }
                }
            )
        })
        .unwrap();
    game.choose(bell).unwrap();
    assert!(matches!(
        game.advance(4096, &mut events).unwrap(),
        n::Status::Decision(_)
    ));
    assert_eq!(
        game.position().top(Player::One, n::Color::Purple),
        Some(CardId(1))
    );
    assert_eq!(
        player_view(&game, Player::One, 0).forecast,
        vec![face_of(&game, CardId(2))]
    );
    let promoted = events
        .iter()
        .find(|e| {
            matches!(e,
                Event::Moved(m) if m.card == CardId(1) && m.source == Zone::Forecast(Player::One)
            )
        })
        .unwrap();
    assert!(
        sentence(&game, promoted, 1).contains(&Seg::Card(face_of(&game, CardId(1)))),
        "the promoted card is public once it is melded"
    );
}

#[test]
fn calicut_return_options_show_the_public_packet_until_each_card_is_returned() {
    for expansions in MODES.into_iter().filter(|e| e.cities()) {
        for tracked in [false, true] {
            let deck = Zone::Deck(n::Deck::base(4));
            let cards = vec![
                n::Card::from_code("C040").unwrap(), // Calicut: Search for Health.
                n::Card::from_code("B040").unwrap(), // Existing green pile.
                n::Card::from_code("B039").unwrap(), // Perspective: outside the packet.
                n::Card::from_code("B036").unwrap(), // Colonialism: return.
                n::Card::from_code("B037").unwrap(), // Gunpowder: return.
                n::Card::from_code("B038").unwrap(), // Anatomy: take into hand.
                n::Card::from_code("B041").unwrap(), // Navigation: return.
            ];
            let placements = [
                (CardId(0), Zone::Hand(Player::One)),
                (CardId(1), Zone::Board(Player::One, n::Color::Green)),
                (CardId(2), deck),
                (CardId(3), deck),
                (CardId(4), deck),
                (CardId(5), deck),
                (CardId(6), deck),
            ];
            let mut game = Game::new(
                n::Position::new(cards, &placements, Player::One).unwrap(),
                expansions,
            )
            .unwrap();
            if tracked {
                game = game.with_tracking().unwrap();
            }
            assert_eq!(game.city_search_returns().count(), 0);
            game.take_action(Action::Meld { card: CardId(0) }, n::ActionKind::Regular)
                .unwrap();
            let mut events = Vec::new();
            let n::Status::Decision(mut menu) = game.advance(4096, &mut events).unwrap() else {
                panic!("Calicut must ask for the Search return order");
            };
            assert_eq!(
                events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Revealed { card, .. } => Some(*card),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                [CardId(6), CardId(5), CardId(4), CardId(3)]
            );
            for (pending, returned) in [
                (vec![CardId(6), CardId(4), CardId(3)], CardId(4)),
                (vec![CardId(6), CardId(3)], CardId(6)),
            ] {
                for viewer in 0..2 {
                    let shown = effect_decision(&game, &menu, viewer, &BTreeSet::new());
                    assert_eq!(shown.options.len(), pending.len());
                    for (option, &card) in shown.options.iter().zip(&pending) {
                        let id = face_of(&game, card);
                        assert_eq!(option.card, Some(id));
                        assert!(option.segs.contains(&Seg::Card(id)));
                        assert!(!option.segs.iter().any(|seg| matches!(seg, Seg::Back(..))));
                    }
                    // Previously returned and never-revealed deck cards
                    // do not gain permission through a Known descriptor.
                    for &card in &[CardId(2), CardId(4)] {
                        if pending.contains(&card) {
                            continue;
                        }
                        let hidden = Options::new(&game, viewer, menu.player)
                            .target("Return ", &TargetDescriptor::Known { card, source: deck });
                        assert_eq!(hidden.card, None);
                        assert!(hidden.segs.contains(&Seg::Back(Set::Base, 4)));
                    }
                }
                let chosen = menu
                    .options
                    .iter()
                    .position(|option| {
                        matches!(option,
                            Alternative::Move {
                                verb: Verb::Return,
                                target: TargetDescriptor::Known { card, .. },
                            } if *card == returned
                        )
                    })
                    .unwrap();
                game.choose(chosen).unwrap();
                match game.advance(4096, &mut events).unwrap() {
                    n::Status::Decision(next) => menu = next,
                    n::Status::Complete(_) => assert_eq!(pending.len(), 2),
                    status => panic!("unexpected Search status: {status:?}"),
                }
            }
            assert_eq!(game.city_search_returns().count(), 0);
            assert_eq!(
                game.position().cards(deck),
                [CardId(3), CardId(6), CardId(4), CardId(2)]
            );
            assert_eq!(game.position().cards(Zone::Hand(Player::One)), [CardId(5)]);
            for viewer in 0..2 {
                for event in &events {
                    if let Event::Moved(mv) = event
                        && mv.source == deck
                    {
                        assert!(
                            sentence(&game, event, viewer)
                                .contains(&Seg::Card(face_of(&game, mv.card)))
                        );
                    }
                }
                for &card in game.position().cards(deck) {
                    assert!(!visible_cards(&game, viewer).contains(&face_of(&game, card)));
                }
            }
        }
    }
}

fn fixture() -> Game {
    let cards = vec![
        n::Card::from_code("B006").unwrap(),
        n::Card::from_code("B019").unwrap(),
        n::Card::from_code("F001").unwrap(),
        n::Card::from_code("B012").unwrap(),
    ];
    let placements = [
        (CardId(0), Zone::Hand(Player::One)),
        (CardId(1), Zone::Hand(Player::Two)),
        (CardId(2), Zone::Board(Player::One, cards[2].color)),
        (CardId(3), Zone::AvailableAchievements),
    ];
    Game::new(
        n::Position::new(cards, &placements, Player::One).unwrap(),
        n::Expansions::FIGURES,
    )
    .unwrap()
}

#[test]
fn every_alternative_and_turn_option_has_a_readable_label() {
    let game = fixture();
    let options = Options::new(&game, 0, Player::One);
    let target = TargetDescriptor::Known {
        card: CardId(0),
        source: Zone::Hand(Player::One),
    };
    let hidden = TargetDescriptor::Hidden {
        source: Zone::Score(Player::Two),
        slot: 2,
        back: n::Deck::figures(4),
    };
    let cases = vec![
        (
            Alternative::Move {
                verb: Verb::Return,
                target: target.clone(),
            },
            "Return Masonry [1] from your hand",
        ),
        (
            Alternative::Select {
                target: target.clone(),
            },
            "Select Masonry [1] from your hand",
        ),
        (
            Alternative::Reveal {
                target: target.clone(),
            },
            "Reveal Masonry [1] from your hand",
        ),
        (
            Alternative::Select {
                target: hidden.clone(),
            },
            "Select [Figures 4] from your opponent's score pile · #3",
        ),
        (
            Alternative::BlindSelect {
                source: Zone::Score(Player::Two),
                back: n::Deck::base(2),
            },
            "Select a random [Base 2] from your opponent's score pile",
        ),
        (
            Alternative::BlindMove {
                verb: Verb::Junk,
                source: Zone::Hand(Player::Two),
                back: n::Deck::figures(5),
            },
            "Junk a random [Figures 5] from your opponent's hand",
        ),
        (
            Alternative::Draw {
                value: 3,
                set: n::CardSet::Figures,
            },
            "Draw Figures (starting at value 3)",
        ),
        (
            Alternative::Splay {
                player: Player::One,
                color: n::Color::Blue,
                direction: n::Splay::Right,
            },
            "Splay blue right",
        ),
        (
            Alternative::Splay {
                player: Player::Two,
                color: n::Color::Blue,
                direction: n::Splay::Right,
            },
            "Splay your opponent's blue right",
        ),
        (
            Alternative::Splay {
                player: Player::Two,
                color: n::Color::Green,
                direction: n::Splay::None,
            },
            "Unsplay your opponent's green",
        ),
        (Alternative::Value(0), "Value 0"),
        (Alternative::BonusIcon, "Bonus"),
        (
            Alternative::ClaimSpecial(n::Special::Destiny),
            "Claim Destiny",
        ),
        (
            Alternative::Move {
                verb: Verb::Foreshadow,
                target: target.clone(),
            },
            "Foreshadow Masonry [1] from your hand",
        ),
        (
            Alternative::BlindMove {
                verb: Verb::Place,
                source: Zone::Forecast(Player::Two),
                back: n::Deck::base(2),
            },
            "Place a random [Base 2] from your opponent's forecast",
        ),
        (Alternative::Icon(n::Icon::Authority), "Authority"),
        (Alternative::Icon(n::Icon::Food), "Food"),
        (
            Alternative::Colors((1 << n::Color::Blue as u8) | (1 << n::Color::Yellow as u8)),
            "yellow, blue",
        ),
        (Alternative::Branch(2), "Option 3"),
        (
            Alternative::MatchingPair {
                first: target.clone(),
                second: hidden,
            },
            "Select Masonry [1] from your hand and [Figures 4] from your opponent's score pile · #3",
        ),
        (
            Alternative::Exchange {
                left: LogicalZone::Hand(Player::One),
                right: LogicalZone::Score(Player::Two),
            },
            "Exchange your hand with your opponent's score pile",
        ),
        (Alternative::Deck(n::Deck::figures(7)), "Figures 7 deck"),
        (
            Alternative::NameFigure(&n::FIGURE_NAMES[1]),
            "Name Gilgamesh",
        ),
        (Alternative::Pass, "Name no Figure (search fails)"),
        (Alternative::Decline, "Decline"),
        (Alternative::Decree(Decree::War), "Issue the War Decree"),
        (
            Alternative::JunkSpecial(n::Special::Monument),
            "Junk Monument",
        ),
        (
            Alternative::RestoreSpecial(n::Special::Empire),
            "Restore Empire",
        ),
        (
            Alternative::ClaimSpecial(n::Special::Wonder),
            "Claim Wonder",
        ),
        (
            Alternative::SelfExecute(CardId(0)),
            "Self-execute Masonry [1]",
        ),
        (
            Alternative::Karma {
                source: CardId(2),
                figure: n::Figure::Hammurabi,
            },
            "Karma of Hammurabi [1]",
        ),
    ];
    for (a, expected) in &cases {
        assert_eq!(
            plain(&options.alternative(a).segs, 0, false),
            *expected,
            "{a:?}"
        );
    }
    for (action, expected) in [
        (Action::Draw, "Draw (starting at value 1)"),
        (Action::Meld { card: CardId(0) }, "Meld Masonry [1]"),
        (Action::Achieve { card: CardId(3) }, "Achieve [Base 1]"),
        (
            Action::Dogma {
                card: CardId(0),
                auspice: false,
            },
            "Dogma Masonry [1]",
        ),
        (
            Action::Dogma {
                card: CardId(0),
                auspice: true,
            },
            "Dogma Masonry [1] with Auspice",
        ),
        (Action::Decree, "Issue a Decree"),
    ] {
        let option = options.action(action, Player::One);
        assert_eq!(plain(&option.segs, 0, false), expected);
        if matches!(action, Action::Achieve { .. }) {
            assert_eq!(option.card, None);
        }
    }
    assert_eq!(
        decision_prompt(&[cases[0].0.clone(), Alternative::Decline]),
        "Choose a card to return"
    );
    assert_eq!(
        decision_prompt(&[Alternative::Colors(1), Alternative::Colors(2)]),
        "Choose a color"
    );
    assert_eq!(
        decision_prompt(&[Alternative::Icon(n::Icon::Concept), Alternative::BonusIcon]),
        "Choose an icon"
    );
    assert_eq!(
        decision_prompt(&[Alternative::Value(1), Alternative::Value(2)]),
        "Choose a value"
    );
    let karma = &cases
        .iter()
        .find(|(a, _)| matches!(a, Alternative::Karma { .. }))
        .unwrap()
        .0;
    assert_eq!(
        decision_prompt(std::slice::from_ref(karma)),
        "Choose which karma applies"
    );
    assert_eq!(
        decision_prompt(&[cases[0].0.clone(), cases[6].0.clone()]),
        "Choose an option"
    );
    assert_eq!(
        options.alternative(&cases[0].0).card,
        Some(face_of(&game, CardId(0)))
    );
    assert_eq!(options.alternative(&cases[3].0).card, None);
    assert_eq!(
        options.alternative(karma).card,
        Some(face_of(&game, CardId(2)))
    );
}

#[test]
fn log_nesting_follows_dogmas_executions_and_karma_bodies() {
    let moved = |inside_karma| {
        Event::Moved(Movement {
            verb: None,
            card: CardId(0),
            source: Zone::Deck(n::Deck::base(1)),
            destination: Zone::Hand(Player::One),
            executor: Player::One,
            inside_karma,
            public: false,
        })
    };
    let started = |inside_karma| Event::ExecutionStarted {
        executor: Player::One,
        card: CardId(0),
        mode: n::ExecutionMode::SelfExecute,
        source: Some(CardId(1)),
        inside_karma,
    };
    let ended = Event::ExecutionEnded {
        executor: Player::One,
        card: CardId(0),
    };
    let chain = Event::ChainAwarded {
        executor: Player::One,
        card: CardId(0),
    };
    let karma = Event::KarmaTriggered {
        owner: Player::One,
        card: CardId(1),
        clause: 0,
        reaction: n::Reaction::First,
        actor: Player::One,
        would: None,
    };
    let chose = Event::Chose {
        player: Player::One,
        choice: Alternative::Value(3),
        label: None,
    };
    let mut nesting = Nesting::default();
    // A Dogma action's effects sit under its line.
    nesting.action(true);
    assert_eq!(nesting.level(&moved(false)), 1);
    // A self-execution's line, then its Chain award and effects one level
    // in; the karma body one more, including the choice announced inside
    // it.
    assert_eq!(nesting.level(&started(false)), 1);
    assert_eq!(nesting.level(&chain), 2);
    assert_eq!(nesting.level(&moved(false)), 2);
    assert_eq!(nesting.level(&karma), 2);
    assert_eq!(nesting.level(&chose), 3);
    assert_eq!(nesting.level(&moved(true)), 3);
    // A declined karma leaves no body: the next fact closes it.
    assert_eq!(nesting.level(&karma), 2);
    assert_eq!(nesting.level(&moved(false)), 2);
    // Nested executions, including one a karma body asks for.
    assert_eq!(nesting.level(&started(false)), 2);
    assert_eq!(nesting.level(&karma), 3);
    assert_eq!(nesting.level(&started(true)), 4);
    assert_eq!(nesting.level(&moved(true)), 5);
    // An end writes no line; it only closes its execution's level.
    assert_eq!(nesting.level(&ended), 3);
    assert_eq!(nesting.level(&ended), 2);
    assert_eq!(nesting.level(&moved(false)), 2);
    assert_eq!(nesting.level(&ended), 1);
    // Other actions and turns reset it.
    nesting.action(false);
    assert_eq!(nesting.level(&moved(false)), 0);
    assert_eq!(Nesting::default(), nesting);
}

#[test]
fn event_sentences_use_event_time_visibility_and_name_specials_and_decrees() {
    let game = fixture();
    let movement = |verb, source, destination| {
        Event::Moved(Movement {
            verb,
            card: CardId(0),
            source,
            destination,
            executor: Player::Two,
            inside_karma: false,
            public: false,
        })
    };
    let cases = [
        (
            movement(None, Zone::Deck(n::Deck::base(1)), Zone::Hand(Player::Two)),
            "AI draws [Base 1]",
        ),
        (
            movement(
                Some(Verb::Meld),
                Zone::Hand(Player::Two),
                Zone::Board(Player::Two, n::Color::Yellow),
            ),
            "AI melds Masonry [1]",
        ),
        (
            movement(
                Some(Verb::Score),
                Zone::Hand(Player::Two),
                Zone::Score(Player::Two),
            ),
            "AI scores [Base 1]",
        ),
        (
            movement(
                Some(Verb::Score),
                Zone::Board(Player::Two, n::Color::Yellow),
                Zone::Score(Player::Two),
            ),
            "AI scores Masonry [1] from their board",
        ),
        (
            movement(
                Some(Verb::Transfer),
                Zone::Hand(Player::Two),
                Zone::Hand(Player::One),
            ),
            "AI transfers Masonry [1] from their hand to your hand",
        ),
        (
            movement(
                Some(Verb::Return),
                Zone::Hand(Player::Two),
                Zone::Deck(n::Deck::base(1)),
            ),
            "AI returns [Base 1]",
        ),
        (
            movement(Some(Verb::Junk), Zone::Score(Player::Two), Zone::Junk),
            "AI junks [Base 1] from their score pile",
        ),
        (
            movement(
                Some(Verb::Tuck),
                Zone::Hand(Player::Two),
                Zone::Board(Player::Two, n::Color::Yellow),
            ),
            "AI tucks Masonry [1]",
        ),
        (
            Event::Revealed {
                card: CardId(0),
                source: Zone::Hand(Player::Two),
                executor: Player::Two,
                inside_karma: true,
            },
            "AI reveals Masonry [1] from their hand",
        ),
        (
            Event::Splayed {
                player: Player::Two,
                color: n::Color::Red,
                from: n::Splay::Left,
                to: n::Splay::Right,
                executor: Player::Two,
                inside_karma: false,
            },
            "AI splays red right",
        ),
        (
            Event::SpecialMoved {
                special: n::Special::Monument,
                source: SpecialLocation::Available,
                destination: SpecialLocation::Owned(Player::One),
                executor: Player::One,
                inside_karma: false,
            },
            "You claim Monument",
        ),
        (
            Event::DecreeMoved {
                decree: Decree::War,
                source: DecreeLocation::Supply,
                destination: DecreeLocation::Owned(Player::Two),
                executor: Player::Two,
                inside_karma: true,
            },
            "AI issues the War Decree",
        ),
        (
            Event::AgeZeroIntroduced {
                executor: Player::One,
                count: 15,
            },
            "Age 0 is introduced",
        ),
        (
            Event::Shuffled {
                executor: Player::One,
                deck: n::Deck::figures(4),
            },
            "You shuffle the Figures 4 deck",
        ),
    ];
    for (event, expected) in &cases {
        assert_eq!(plain(&sentence(&game, event, 0), 0, false), *expected);
    }
    let mut nesting = Nesting::default();
    assert_eq!(nesting.level(&cases[8].0), 1);
    assert_eq!(nesting.level(&cases[11].0), 1);
    assert_eq!(nesting.level(&cases[9].0), 0);
    assert_eq!(nesting.level(&cases[12].0), 1);
    assert_eq!(nesting.level(&cases[13].0), 1);
    let search = Event::FigureSearch {
        executor: Player::Two,
        deck: n::Deck::figures(1),
        name: Some(&n::FIGURE_NAMES[0]),
        found: Some(CardId(2)),
    };
    assert_eq!(
        plain(&sentence(&game, &search, 0), 0, false),
        "AI names Hammurabi in the Figures 1 deck (found)"
    );
    assert!(
        sentence(&game, &search, 0)
            .iter()
            .all(|seg| !matches!(seg, Seg::Card(_)))
    );
    assert_eq!(Nesting::default().level(&search), 1);
    let exchange = Event::Exchanged {
        executor: Player::One,
        inside_karma: false,
        movements: vec![
            n::ExchangeMovement {
                card: CardId(0),
                source: Zone::Hand(Player::One),
                destination: Zone::Score(Player::Two),
            },
            n::ExchangeMovement {
                card: CardId(1),
                source: Zone::Score(Player::Two),
                destination: Zone::Hand(Player::One),
            },
        ],
    };
    let sentence = sentence(&game, &exchange, 0);
    assert_eq!(
        sentence
            .iter()
            .filter(|s| matches!(s, Seg::Card(_)))
            .count(),
        2
    );
    assert!(
        plain(&sentence, 0, false)
            .starts_with("You exchange Masonry [1] from your hand to your opponent's score pile; ")
    );
}

#[test]
fn log_turn_headers_gaps_and_folding_follow_regular_actions_not_decisions() {
    let mut m = dealt(n::Expansions::BASE, 42);
    let first = m.game().position().turn_owner();
    for (turn, action, actions) in [
        (1, 1, 1),
        (2, 1, 2),
        (2, 2, 2),
        (3, 1, 2),
        (3, 2, 2),
        (4, 1, 2),
    ] {
        let v = view(&m, index(first), false, false);
        assert_eq!((v.turn, v.action, v.actions), (turn, action, actions));
        let headers: Vec<_> = v
            .log
            .iter()
            .filter_map(|e| match e {
                Entry::Header(t, current) => Some((t.as_str(), *current)),
                _ => None,
            })
            .collect();
        assert_eq!(headers[0], ("Setup", false));
        assert_eq!(
            headers.last().unwrap().0,
            format!("Turn {turn}: {}", if turn % 2 == 1 { "You" } else { "AI" })
        );
        assert!(headers.last().unwrap().1);
        assert_eq!(headers.iter().filter(|(_, current)| *current).count(), 1);
        let completed_second_actions = m
            .record()
            .steps
            .iter()
            .filter(|s| matches!(s, Step::Action(_)))
            .count()
            .saturating_sub(1)
            / 2;
        assert_eq!(
            v.log.iter().filter(|e| matches!(e, Entry::Gap)).count(),
            completed_second_actions
        );
        m.apply(Step::Action(Action::Draw)).unwrap();
    }
    let mut out = vec![Entry::Header("Setup".into(), false)];
    line(&mut out, 1, vec![text("same")]);
    line(&mut out, 1, vec![text("same")]);
    line(&mut out, 0, vec![text("same")]);
    out.push(Entry::Gap);
    line(&mut out, 0, vec![text("same")]);
    assert_eq!(out[1], Entry::Line(1, vec![text("same")], 2));
    assert_eq!(out.len(), 5);
}

#[test]
fn every_decree_has_printed_text_for_its_tooltip() {
    for d in [
        Decree::War,
        Decree::Expansion,
        Decree::Trade,
        Decree::Advancement,
        Decree::Rivalry,
    ] {
        assert!(
            !super::super::table::decree_text(decree(d)).is_empty(),
            "{d:?}"
        );
    }
    assert!(super::super::table::decree_text("Advancement").contains("two higher"));
}

#[test]
fn subgame_turns_keep_their_own_clock_and_headers() {
    // Random play through the session layer on this seed starts and
    // finishes one Martian Internet subgame, then the main game plays on.
    const SEED: u64 = 245;
    let mut m = Match::with_subgames(n::Expansions::ARTIFACTS, SEED, true).unwrap();
    let mut k = 0;
    let mut main_turn_at_start = None;
    let mut inside_turns = Vec::new();
    let mut resumed = false;
    while !matches!(m.prompt(), Prompt::Over(_)) {
        let step = m.random_step(SEED * 100_003 + k).unwrap();
        k += 1;
        m.apply(step).unwrap();
        let v = view(&m, 0, false, false);
        let headers: Vec<_> = v
            .log
            .iter()
            .filter_map(|e| match e {
                Entry::Header(t, _) => Some(t.clone()),
                _ => None,
            })
            .collect();
        if m.game().subgame_depth() > 0 {
            if main_turn_at_start.is_none() {
                main_turn_at_start = headers
                    .iter()
                    .rev()
                    .find(|h| h.starts_with("Turn "))
                    .cloned();
            }
            let main_turn = main_turn_at_start
                .as_deref()
                .expect("a main-game turn before the subgame");
            inside_turns.push(v.turn);
            assert!(headers.iter().any(|h| h == main_turn));
            if let Some(last) = headers.last() {
                assert!(
                    last.starts_with("Subgame 1 · Turn ") || last.contains("start"),
                    "{last}"
                );
            }
        } else if !inside_turns.is_empty() && !resumed {
            resumed = true;
            // The interrupted turn resumes with its own number.
            let main_turn = main_turn_at_start.clone().unwrap();
            assert_eq!(
                v.turn,
                main_turn["Turn ".len()..main_turn.find(':').unwrap()]
                    .parse::<usize>()
                    .unwrap()
            );
            assert_eq!(headers.last().unwrap(), &format!("{main_turn} (continued)"));
        }
    }
    assert!(resumed, "seed {SEED} must play a whole subgame");
    assert_eq!(inside_turns[0], 1);
    assert!(inside_turns.iter().all(|&t| t <= inside_turns.len()));
    // The main game's turn count ignores the subgame's turns.
    let main_turns = m
        .events()
        .iter()
        .scan(0usize, |depth, (_, e)| {
            match e {
                Event::SubgameStarted { .. } => *depth += 1,
                Event::SubgameEnded { .. } => *depth -= 1,
                _ => {}
            }
            Some((*depth, e))
        })
        .filter(|(depth, e)| *depth == 0 && matches!(e, Event::TurnBegan { .. }))
        .count();
    let v = view(&m, 0, false, false);
    assert_eq!(v.turn, main_turns);
    let last_main = v
        .log
        .iter()
        .rev()
        .find_map(|e| match e {
            Entry::Header(t, _) if t.starts_with("Turn ") && !t.ends_with("(continued)") => {
                Some(t.clone())
            }
            _ => None,
        })
        .unwrap();
    assert!(
        last_main.starts_with(&format!("Turn {main_turns}:")),
        "{last_main}"
    );
}

#[test]
fn figures_effective_counts_bonuses_and_enum_mappings_are_not_physical_totals() {
    let game = fixture();
    assert_eq!(player_view(&game, Player::One, 0).bonus_score, 2);
    assert_eq!(player_view(&game, Player::One, 0).score_total, 2);
    let listed = player_view(&game, Player::One, 0).bonuses;
    assert!(listed.windows(2).all(|w| w[0] >= w[1]), "{listed:?}");
    assert_eq!(
        i64::from(listed[0]) + listed.len() as i64 - 1,
        player_view(&game, Player::One, 0).bonus_score
    );
    let cards = vec![
        n::Card::from_code("F001").unwrap(),
        n::Card::from_code("F002").unwrap(),
    ];
    let placements = [
        (CardId(0), Zone::Board(Player::One, n::Color::Red)),
        (CardId(1), Zone::Board(Player::One, n::Color::Red)),
    ];
    let mut bonuses = Game::new(
        n::Position::new(cards, &placements, Player::One).unwrap(),
        n::Expansions::FIGURES,
    )
    .unwrap();
    assert_eq!(player_view(&bonuses, Player::One, 0).bonus_score, 1);
    bonuses
        .start(
            n::Operation::Splay {
                player: Player::One,
                color: n::Color::Red,
                direction: n::Splay::Left,
            },
            n::Context::new(Player::One),
        )
        .unwrap();
    assert!(matches!(
        bonuses.advance(100, &mut Vec::new()).unwrap(),
        n::Status::Complete(_)
    ));
    // The largest visible bonus (2), plus one for the other bonus (1).
    assert_eq!(player_view(&bonuses, Player::One, 0).bonus_score, 3);
    assert_eq!(
        player_view(&bonuses, Player::One, 0).stacks[0].cards,
        [face_of(&bonuses, CardId(0)), face_of(&bonuses, CardId(1))]
    );
    let cards = vec![
        n::Card::from_code("F002").unwrap(),
        n::Card::from_code("B002").unwrap(),
    ];
    let placements = [
        (CardId(0), Zone::Board(Player::One, cards[0].color)),
        (CardId(1), Zone::Board(Player::Two, cards[1].color)),
    ];
    let game = Game::new(
        n::Position::new(cards, &placements, Player::One).unwrap(),
        n::Expansions::FIGURES,
    )
    .unwrap();
    let own = player_view(&game, Player::One, 0);
    assert_eq!(
        own.icons[0],
        game.position()
            .basic_icon_count(Player::One, n::Icon::Authority)
            * 2
    );
    // A negative Figures score is shown as it is, not clamped to zero.
    assert!(game.position().score(Player::Two) < 0);
    assert_eq!(
        player_view(&game, Player::Two, 0).score_total,
        game.position().score(Player::Two)
    );
}

#[test]
fn tools_prompts_name_which_effect_is_asking() {
    // Tools: "You may return three cards from your hand. If you do, draw
    // and meld a [3]. / You may return a [3] from your hand. If you do,
    // draw three [1]." Declining the first question must visibly move
    // on to the second, which asks for a card the same way.
    let tools = n::Card::from_code("B011").unwrap();
    let color = tools.color;
    let mut cards = vec![tools];
    cards.extend(["B001", "B002", "B003"].map(|code| n::Card::from_code(code).unwrap()));
    cards.push(n::Card::from_code("B031").unwrap()); // an age-3 card for the hand
    let mut game = Game::new(
        n::Position::new(
            cards,
            &[
                (CardId(0), Zone::Board(Player::One, color)),
                (CardId(1), Zone::Hand(Player::One)),
                (CardId(2), Zone::Hand(Player::One)),
                (CardId(3), Zone::Hand(Player::One)),
                (CardId(4), Zone::Hand(Player::One)),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::BASE,
    )
    .unwrap();
    game.take_action(
        Action::Dogma {
            card: CardId(0),
            auspice: false,
        },
        n::ActionKind::Regular,
    )
    .unwrap();
    let mut prompts = Vec::new();
    while let n::Status::Decision(menu) = game.advance(4096, &mut Vec::new()).unwrap() {
        let d = effect_decision(&game, &menu, 0, &BTreeSet::new());
        prompts.push(plain(&d.prompt, 0, false));
        let decline = menu
            .options
            .iter()
            .position(|o| matches!(o, Alternative::Decline))
            .expect("both Tools effects are optional");
        game.choose(decline).unwrap();
    }
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert_eq!(
        prompts,
        [
            "Tools [1] · 1st effect: Choose a card to return",
            "Tools [1] · 2nd effect: Choose a card to return"
        ]
    );
}

#[test]
fn native_newton_prompt_names_the_source_and_both_board_destinations() {
    let newton = n::Card::from_code("F052").unwrap();
    let color = newton.color;
    let cards = vec![newton, n::Card::from_code("B004").unwrap()];
    let mut game = Game::new(
        n::Position::new(
            cards,
            &[
                (CardId(0), Zone::Board(Player::One, color)),
                (CardId(1), Zone::Deck(n::Deck::base(1))),
            ],
            Player::One,
        )
        .unwrap(),
        n::Expansions::FIGURES,
    )
    .unwrap();
    game.take_action(Action::Draw, n::ActionKind::Regular)
        .unwrap();
    let n::Status::Decision(menu) = game.advance(4096, &mut Vec::new()).unwrap() else {
        panic!("Newton's transfer choice");
    };
    let d = effect_decision(&game, &menu, 0, &BTreeSet::new());
    assert!(matches!(d.prompt[0], Seg::Card(id) if id == face_of(&game, CardId(0))));
    assert!(plain(&d.prompt, 0, false).contains("Isaac Newton"));
    assert_eq!(
        plain(&d.options[0].segs, 0, false),
        "Transfer the drawn card to your board"
    );
    assert_eq!(
        plain(&d.options[1].segs, 0, false),
        "Transfer the drawn card to your opponent's board"
    );
    assert_eq!(d.options.len(), menu.options.len());
}

#[test]
fn executed_effect_wording_and_win_source_are_public_after_it_returns() {
    // Use a real face with a small synthetic continuation: return its own
    // source, ask the printed branches, then win. Executing a hand card
    // exposes its text at the table, so both seats keep its name and
    // wording after it leaves for the deck.
    let mut card = n::Card::from_code("B018").unwrap();
    card.effects[0].instructions.insert(
        0,
        n::program::build::return_(n::Selector::Reg(n::Reg::Source)),
    );
    card.effects[0].instructions.truncate(2);
    card.effects[0]
        .instructions
        .push(n::Instruction::Win(n::WinRule::Executor));
    let mut game = Game::new(
        n::Position::new(
            vec![card],
            &[(CardId(0), Zone::Hand(Player::One))],
            Player::One,
        )
        .unwrap(),
        n::Expansions::BASE,
    )
    .unwrap();
    game.self_execute(CardId(0), n::Context::new(Player::One))
        .unwrap();
    let n::Status::Decision(menu) = game.advance(4096, &mut Vec::new()).unwrap() else {
        panic!("private branch");
    };
    let own = effect_decision(&game, &menu, 0, &BTreeSet::new());
    let other = effect_decision(&game, &menu, 1, &BTreeSet::new());
    assert!(plain(&own.prompt, 0, false).contains("Canal Building"));
    assert!(plain(&other.prompt, 1, false).contains("Canal Building"));
    assert!(plain(&own.options[0].segs, 0, false).starts_with("Exchange all highest"));
    assert!(plain(&other.options[0].segs, 1, false).starts_with("Exchange all highest"));
    game.choose(0).unwrap();
    assert_eq!(
        game.advance(4096, &mut Vec::new()).unwrap(),
        n::Status::Winner(Player::One)
    );
    let prompt = Prompt::Over(n::MatchOutcome::Winner {
        player: Player::One,
        victory: game.victory().unwrap(),
    });
    assert_eq!(
        outcome(&prompt, &game, 0),
        Some((Outcome::Winner(0), "Card effect: Canal Building".into()))
    );
    assert_eq!(
        outcome(&prompt, &game, 1),
        Some((Outcome::Winner(0), "Card effect: Canal Building".into()))
    );
}

#[test]
fn terminal_reasons_are_only_those_the_native_prompt_actually_reports() {
    let mut game = Game::new_game(1, n::Expansions::BASE).unwrap();
    let mut events = Vec::new();
    while let n::Status::Decision(_) = game.advance(4096, &mut events).unwrap() {
        game.choose(0).unwrap();
    }
    for (reason, expected) in [
        (n::WinReason::Achievements, "Achievements"),
        (
            n::WinReason::Exhaustion,
            "Score after exhaustion (achievements break ties)",
        ),
        (n::WinReason::Effect, "Card effect"),
        (n::WinReason::Karma, "Karma"),
    ] {
        let mut game = game.clone();
        game.start(
            n::Operation::ProposeWin {
                winner: Player::Two,
                reason,
            },
            n::Context::new(Player::Two),
        )
        .unwrap();
        assert_eq!(
            game.advance(4096, &mut events).unwrap(),
            n::Status::Winner(Player::Two)
        );
        assert_eq!(
            outcome(
                &Prompt::Over(n::MatchOutcome::Winner {
                    player: Player::Two,
                    victory: game.victory().unwrap(),
                }),
                &game,
                0
            ),
            Some((Outcome::Winner(1), expected.into())),
        );
    }
    for (reason, expected) in [
        (n::DrawReason::Exhaustion, "Score after exhaustion"),
        (n::DrawReason::Effect, "Card effect"),
        (n::DrawReason::Loop, "Endless loop"),
    ] {
        assert_eq!(
            outcome(&Prompt::Over(n::MatchOutcome::Draw(reason)), &game, 0),
            Some((Outcome::Draw, expected.into()))
        );
    }
}

#[test]
fn held_specials_decrees_and_virtual_achievements_use_native_counts() {
    // Mandela supplies a virtual achievement per two basic Avatars.
    let cards = vec![
        n::Card::from_code("F086").unwrap(), // Mandela, red
        n::Card::from_code("F005").unwrap(), // Shennong, yellow
    ];
    let placements = [
        (CardId(0), Zone::Board(Player::One, cards[0].color)),
        (CardId(1), Zone::Board(Player::One, cards[1].color)),
    ];
    let mut game = Game::new(
        n::Position::new(cards, &placements, Player::One).unwrap(),
        n::Expansions::FIGURES,
    )
    .unwrap();
    assert_eq!(player_view(&game, Player::One, 0).achievement_count, 1);
    for op in [
        n::Operation::ClaimSpecial {
            special: n::Special::Monument,
            check_eligibility: false,
        },
        n::Operation::ClaimDecree {
            decree: Decree::War,
        },
    ] {
        game.start(op, n::Context::new(Player::One)).unwrap();
        assert!(matches!(
            game.advance(100, &mut Vec::new()).unwrap(),
            n::Status::Complete(_)
        ));
    }
    let p = player_view(&game, Player::One, 0);
    assert_eq!(p.achievement_count, 3);
    assert_eq!(p.achievements, [Held::Special(n::Special::Monument)]);
    assert_eq!(p.decrees, [("War".to_owned(), n::Color::Red)]);
}
