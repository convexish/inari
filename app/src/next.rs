//! The replacement engine's side of the table (Base with optional Echoes,
//! Figures and Cities): the view snapshot, decision options as match steps, and its log.
use std::collections::BTreeSet;

use inno_engine::Icon;
use inno_engine::{
    self as n, Action, Alternative, CardId, Decree, DecreeLocation, Event, Game, LogicalZone,
    Match, Movement, Net, Player, Position, Prompt, Rng, SpecialLocation, Step, TargetDescriptor,
    Tracker, Verb, Zone,
};

use super::faces::{COLORS, FaceId, ICONS, Lead, Set, by_code, face, paragraphs, stack_index};
use super::table::{
    Claimable, DecisionView, DeckRow, Entry, Held, OptionView, Outcome, PlayerView, Secret, Seg,
    StackView, SupplyView, TableView, name, title_case,
};

/// The face of an engine-next card, by publisher code.
pub fn face_of(game: &Game, card: CardId) -> FaceId {
    face_in(game.position(), card)
}

fn face_in(pos: &Position, card: CardId) -> FaceId {
    let code = pos.card(card).code;
    by_code(code).unwrap_or_else(|| panic!("no face for {code}"))
}

pub fn player(index: usize) -> Player {
    Player::ALL[index]
}

pub fn index(player: Player) -> usize {
    Player::ALL.iter().position(|&p| p == player).unwrap_or(0)
}

/// The step each option of the view's decision takes, in the same order:
/// a decision's options, or the turn's legal actions.
pub fn option_steps(m: &Match) -> Vec<Step> {
    match m.prompt() {
        Prompt::Decision(decision) => (0..decision.options.len()).map(Step::Choice).collect(),
        Prompt::Turn { actions, .. } => actions.iter().copied().map(Step::Action).collect(),
        Prompt::Over(_) => Vec::new(),
    }
}

/// The network's greedy step at the match's prompt: its best option, which
/// the observation lists in the prompt's own order. None when the game is
/// over or the observation is refused.
pub fn greedy_step(m: &Match, net: &Net) -> Option<Step> {
    let player = m.to_move()?;
    let observation = m.game().observe(player).ok()?;
    let index = net.greedy(&observation)?;
    option_steps(m).get(index).copied()
}

/// The network's policy step at the match's prompt: an option drawn from
/// `softmax(policy / temperature)` (0 plays its best option, higher
/// temperatures are the easier difficulties, infinity is random play) with
/// a generator seeded from `seed` and the match's position. None for a
/// network without a policy output, when the game is over or the
/// observation is refused; the caller falls back to the greedy Q.
pub fn policy_step(m: &Match, net: &Net, temperature: f64, seed: u64) -> Option<Step> {
    let player = m.to_move()?;
    let observation = m.game().observe(player).ok()?;
    let mut rng =
        Rng::new(seed ^ (m.record().steps.len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let index = net.sample_policy(&observation, temperature, &mut rng)?;
    option_steps(m).get(index).copied()
}

/// The Achieve action for a supply click on an available achievement with
/// this public back, if it is legal now.
pub fn achieve_step(m: &Match, age: u8, set: Set) -> Option<Step> {
    let Prompt::Turn { actions, .. } = m.prompt() else {
        return None;
    };
    actions.iter().find_map(|&action| match action {
        Action::Achieve { card } => {
            let f = face(face_of(m.game(), card));
            (f.age == age && f.set == set).then_some(Step::Action(action))
        }
        _ => None,
    })
}

/// The Achieve action for a Safe click on `seat`'s secret at `position`, if
/// claiming it is legal now.
pub fn secret_step(m: &Match, seat: usize, position: usize) -> Option<Step> {
    let Prompt::Turn { actions, .. } = m.prompt() else {
        return None;
    };
    let &card = m
        .game()
        .position()
        .cards(Zone::Safe(player(seat)))
        .get(position)?;
    actions
        .iter()
        .find(|&&action| action == Action::Achieve { card })
        .map(|&action| Step::Action(action))
}

/// The deal's expansions, as the setup screen names them.
pub fn mode_name(expansions: n::Expansions) -> String {
    let mut name = String::from("Base");
    for (enabled, part) in [
        (expansions.echoes(), "Echoes"),
        (expansions.figures(), "Figures"),
        (expansions.cities(), "Cities"),
        (expansions.unseen(), "Unseen"),
        (expansions.artifacts(), "Artifacts"),
    ] {
        if enabled {
            name.push_str(" + ");
            name.push_str(part);
        }
    }
    name
}

/// The table as `viewer` sees it.
/// `peek` shows the suspended parent's boards during a Martian Internet
/// subgame (nothing can be chosen on them); otherwise the live game's.
pub fn view(m: &Match, viewer: usize, two_player: bool, peek: bool) -> TableView {
    let (turn, action, actions) = clock(m);
    let expansions = m.record().expansions;
    let game = m.game();
    let subgame = game.subgame_depth();
    let peeking = peek && subgame > 0;
    let mode = match (subgame, peeking) {
        (0, _) => mode_name(expansions),
        (depth, false) => format!("{} · subgame {depth}", mode_name(expansions)),
        (depth, true) => format!("{} · parent of subgame {depth}", mode_name(expansions)),
    };
    let players = Player::ALL.map(|p| {
        if peeking {
            let pos = game.parent_position(1).expect("a suspended parent");
            player_view_in(pos, game.parent_tracker(1, player(viewer)), &[], p, viewer)
        } else {
            player_view(game, p, viewer)
        }
    });
    TableView {
        mode,
        figures: expansions.figures(),
        echoes: expansions.echoes(),
        cities: expansions.cities(),
        unseen: expansions.unseen(),
        artifacts: expansions.artifacts(),
        age_zero: m.game().position().age_zero_introduced(),
        over: outcome(m.prompt(), m.game(), viewer),
        current_player: index(m.game().position().turn_owner()),
        turn,
        action,
        actions,
        achievements_needed: 6 + expansions.count(),
        subgame,
        peeking,
        martian_internet_printed: game.subgames(),
        players,
        private_owner: viewer,
        supply: supply(m, viewer),
        decision: if peeking { None } else { decision(m, viewer) },
        log: log_entries(m, viewer, two_player),
    }
}

fn decree(d: Decree) -> &'static str {
    match d {
        Decree::War => "War",
        Decree::Expansion => "Expansion",
        Decree::Trade => "Trade",
        Decree::Advancement => "Advancement",
        Decree::Rivalry => "Rivalry",
    }
}

/// One-based regular-action ordinal -> turn and action within it. Setup
/// commitments and nested free actions do not advance the regular clock.
fn action_clock(ordinal: usize) -> (usize, usize, usize) {
    if ordinal <= 1 {
        (1, 1, 1)
    } else {
        (2 + (ordinal - 2) / 2, 1 + (ordinal - 2) % 2, 2)
    }
}

/// Regular actions started at each nesting level: the main game first, then
/// each Martian Internet subgame still open, the one being played last. A
/// subgame's turns leave the games it interrupted where they were.
fn actions_started(m: &Match) -> Vec<usize> {
    let mut levels = vec![0];
    let mut events = m.events().iter().peekable();
    for tag in 0..=m.record().steps.len() {
        if let Some(Step::Action(_)) = tag.checked_sub(1).and_then(|i| m.record().steps.get(i)) {
            *levels.last_mut().expect("the main game") += 1;
        }
        while events.peek().is_some_and(|(step, _)| *step == tag) {
            match events.next().expect("peeked event").1 {
                Event::SubgameStarted { .. } => levels.push(0),
                Event::SubgameEnded { .. } => {
                    levels.pop();
                }
                _ => {}
            }
        }
    }
    levels
}

fn clock(m: &Match) -> (usize, usize, usize) {
    let started = *actions_started(m).last().expect("the main game");
    // A Turn menu is for the next action; a suspended decision (including
    // action-end fading) or terminal result still belongs to the started one.
    action_clock(started + usize::from(matches!(m.prompt(), Prompt::Turn { .. })))
}

fn outcome(prompt: &Prompt, game: &Game, viewer: usize) -> Option<(Outcome, String)> {
    match prompt {
        Prompt::Over(n::MatchOutcome::Winner { player: p, victory }) => {
            let mut reason = match victory.reason {
                n::WinReason::Achievements => "Achievements".to_owned(),
                n::WinReason::Exhaustion => {
                    "Score after exhaustion (achievements break ties)".to_owned()
                }
                n::WinReason::Effect => "Card effect".to_owned(),
                n::WinReason::Karma => "Karma".to_owned(),
                n::WinReason::Elimination { loser } => {
                    format!("Player {} eliminated", index(loser) + 1)
                }
            };
            if matches!(
                victory.reason,
                n::WinReason::Effect | n::WinReason::Karma | n::WinReason::Elimination { .. }
            ) && let Some(card) = victory.source_card(player(viewer))
            {
                reason.push_str(": ");
                reason.push_str(&title_case(&face(face_of(game, card)).title));
            }
            Some((Outcome::Winner(index(*p)), reason))
        }
        Prompt::Over(n::MatchOutcome::Draw(reason)) => Some((
            Outcome::Draw,
            match reason {
                n::DrawReason::Exhaustion => "Score after exhaustion",
                n::DrawReason::Effect => "Card effect",
                n::DrawReason::Loop => "Endless loop",
            }
            .into(),
        )),
        _ => None,
    }
}

fn sorted(pos: &Position, cards: &[CardId]) -> Vec<FaceId> {
    let mut out: Vec<_> = cards.iter().map(|&c| face_in(pos, c)).collect();
    out.sort_by_key(|&id| {
        let f = face(id);
        (f.age, f.set, stack_index(f.color))
    });
    out
}

fn player_view(game: &Game, p: Player, viewer: usize) -> PlayerView {
    let claimable: Vec<_> = game
        .legal_actions()
        .into_iter()
        .filter_map(|action| match action {
            Action::Achieve { card } => Some(card),
            _ => None,
        })
        .collect();
    player_view_in(
        game.position(),
        game.tracker(player(viewer)),
        &claimable,
        p,
        viewer,
    )
}

/// A player's side of this position; `tracker` is the viewer's beliefs
/// about it and `claimable` the secrets a legal action can claim.
/// The board's visible bonus values, highest first.
fn visible_bonuses(pos: &Position, p: Player) -> Vec<u8> {
    let counts = pos.visible_bonuses(p);
    (0..counts.len())
        .rev()
        .flat_map(|value| std::iter::repeat_n(value as u8, usize::from(counts[value])))
        .collect()
}

fn player_view_in(
    pos: &Position,
    tracker: Option<&Tracker>,
    claimable: &[CardId],
    p: Player,
    viewer: usize,
) -> PlayerView {
    let mut achievements: Vec<_> = pos
        .cards(Zone::Achievements(p))
        .iter()
        .map(|&c| Held::Card(face_in(pos, c)))
        .collect();
    achievements.extend(
        pos.specials()
            .filter(|&s| pos.special_location(s) == SpecialLocation::Owned(p))
            .map(Held::Special),
    );
    // Museums are unnumbered in engine-next: stored ones count from one,
    // claimed ones follow.
    let stored = pos.cards(Zone::Museum(p)).len();
    achievements.extend((0..pos.claimed_museums(p)).map(|i| Held::Museum((stored + i + 1) as u8)));
    // A secret shows its back, and the front when this viewer's tracker has
    // pinned it (they saw it arrive, or it was revealed in place).
    let secrets = tracker
        .map(|tracker| tracker.belief(Zone::Safe(p)))
        .unwrap_or_default();
    PlayerView {
        stacks: COLORS.map(|c| StackView {
            cards: pos
                .cards(Zone::Board(p, c))
                .iter()
                .map(|&id| face_in(pos, id))
                .collect(),
            splay: pos.splay(p, c),
        }),
        hand: sorted(pos, pos.cards(Zone::Hand(p))),
        score: sorted(pos, pos.cards(Zone::Score(p))),
        forecast: sorted(pos, pos.cards(Zone::Forecast(p))),
        hand_visible: pos.can_view(Zone::Hand(p), player(viewer)),
        score_visible: pos.can_view(Zone::Score(p), player(viewer)),
        forecast_visible: pos.can_view(Zone::Forecast(p), player(viewer)),
        forecast_limit: pos.forecast_limit(p),
        safe: pos
            .cards(Zone::Safe(p))
            .iter()
            .enumerate()
            .map(|(slot, &id)| {
                let back = pos.card(id).origin_deck();
                Secret {
                    set: Set::from(back.set),
                    age: back.age,
                    known: secrets
                        .get(slot)
                        .and_then(|belief| belief.known())
                        .map(|card| face_in(pos, card)),
                    legal: pos.turn_owner() == p && claimable.contains(&id),
                }
            })
            .collect(),
        safe_limit: pos.safe_limit(p),
        display: pos
            .cards(Zone::Display(p))
            .first()
            .map(|&id| face_in(pos, id)),
        museums: pos
            .cards(Zone::Museum(p))
            .iter()
            .enumerate()
            .map(|(i, &id)| ((i + 1) as u8, face_in(pos, id)))
            .collect(),
        achievements,
        achievement_count: pos.effective_achievement_count(p),
        // Figures modifiers (Gilgamesh) can make these negative; show them.
        score_total: pos.score(p),
        bonus_score: pos.bonus_score(p),
        bonuses: visible_bonuses(pos, p),
        icons: ICONS.map(|i| pos.icon_count(p, i)),
        // Food counts only once Age 0 is in play.
        food: if pos.age_zero_introduced() {
            pos.icon_count(p, n::Icon::Food)
        } else {
            0
        },
        decrees: Decree::ALL
            .into_iter()
            .filter(|&d| pos.decree_location(d) == DecreeLocation::Owned(p))
            .map(|d| (decree(d).into(), d.color()))
            .collect(),
    }
}

fn supply(m: &Match, viewer: usize) -> SupplyView {
    let game = m.game();
    let pos = game.position();
    let expansions = m.record().expansions;
    let decks: Vec<_> = n::CardSet::ALL
        .into_iter()
        .filter(|&s| expansions.includes(s))
        .collect();
    let decks = decks
        .into_iter()
        .map(|s| DeckRow {
            set: Set::from(s),
            counts: std::array::from_fn(|age| {
                pos.cards(Zone::Deck(n::Deck {
                    set: s,
                    age: age as u8,
                }))
                .len()
            }),
        })
        .collect();
    let mut achievements: Vec<_> = pos
        .cards(Zone::AvailableAchievements)
        .iter()
        .map(|&c| {
            let f = face(face_of(game, c));
            Claimable {
                set: f.set,
                age: f.age,
                legal: m.to_move() == Some(player(viewer))
                    && achieve_step(m, f.age, f.set).is_some(),
            }
        })
        .collect();
    achievements.sort_by_key(|c| (c.age, c.set));
    // Each set's available tokens have their own group.
    let specials: Vec<Vec<_>> = [
        &n::Special::ALL[..],
        &n::Special::ECHOES[..],
        &n::Special::CITIES[..],
        &n::Special::UNSEEN[..],
    ]
    .into_iter()
    .map(|group| {
        group
            .iter()
            .copied()
            .filter(|&s| pos.specials().any(|enabled| enabled == s))
            .filter(|&s| pos.special_location(s) == SpecialLocation::Available)
            .collect::<Vec<_>>()
    })
    .filter(|group| !group.is_empty())
    .collect();
    SupplyView {
        decks,
        achievements,
        museums_available: pos.available_museums(),
        museums_junked: pos.junked_museums(),
        junk: pos
            .cards(Zone::Junk)
            .iter()
            .map(|&c| face_of(game, c))
            .collect(),
        junk_visible: pos.can_view(Zone::Junk, player(viewer)),
        junk_specials: pos
            .specials()
            .filter(|&s| pos.special_location(s) == SpecialLocation::Junk)
            .collect(),
        specials,
        decrees: if expansions.figures() {
            Decree::ALL
                .into_iter()
                .filter(|&d| pos.decree_location(d) == DecreeLocation::Supply)
                .map(|d| (decree(d).into(), d.color()))
                .collect()
        } else {
            Vec::new()
        },
    }
}

// Physical zone visibility, deliberately NOT engine target knowledge, logical
// membership (Meiji/Michelangelo/von Braun), or previously revealed identity.
fn visible_zone(zone: Zone, viewer: usize) -> bool {
    match zone {
        // Artifacts on display and in Museums are face up.
        Zone::Board(..) | Zone::Display(_) | Zone::Museum(_) => true,
        Zone::Hand(p) | Zone::Score(p) | Zone::Forecast(p) => index(p) == viewer,
        _ => false,
    }
}

fn visible_cards(game: &Game, viewer: usize) -> BTreeSet<FaceId> {
    let pos = game.position();
    let own = player(viewer);
    // Own private piles, plus the other player's and the junk while the
    // engine lets this viewer look at them (an inspection grant, a lent
    // hand, von Braun's junk).
    let other = own.opponent();
    [Zone::Hand(own), Zone::Score(own), Zone::Forecast(own)]
        .into_iter()
        .chain(
            [
                Zone::Hand(other),
                Zone::Score(other),
                Zone::Forecast(other),
                Zone::Junk,
            ]
            .into_iter()
            .filter(|&zone| pos.can_view(zone, own)),
        )
        .chain(Player::ALL.into_iter().flat_map(|p| {
            n::Color::ALL
                .map(|c| Zone::Board(p, c))
                .into_iter()
                .chain([Zone::Display(p), Zone::Museum(p)])
        }))
        .flat_map(|z| pos.cards(z))
        .copied()
        // Search keeps its revealed packet in the deck during return choices.
        .chain(game.city_search_returns())
        .map(|c| face_of(game, c))
        .collect()
}

/// Cards revealed during the current action, wherever they are now. Every
/// reveal is public, so a later choice in the same action may offer them by
/// name (Blackmail's meld and self-execution, even after Bear Grylls junks
/// them); the engine still decides which options are Known. Earlier reveals
/// show backs.
fn revealed_this_action(m: &Match) -> BTreeSet<CardId> {
    let start = m
        .record()
        .steps
        .iter()
        .rposition(|step| matches!(step, Step::Action(_)))
        .map_or(0, |i| i + 1);
    revealed_cards(
        m.events()
            .iter()
            .filter(|(step, _)| *step >= start)
            .map(|(_, event)| event),
    )
}

fn revealed_cards<'a>(events: impl Iterator<Item = &'a Event>) -> BTreeSet<CardId> {
    events
        .filter_map(|event| match event {
            Event::Revealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

fn back(id: FaceId) -> Seg {
    let f = face(id);
    Seg::Back(f.set, f.age)
}

fn text(s: impl Into<String>) -> Seg {
    Seg::Text(s.into())
}
fn strong(s: impl Into<String>) -> Seg {
    Seg::Strong(s.into())
}

fn verb(v: Verb) -> &'static str {
    match v {
        Verb::Meld => "meld",
        Verb::Score => "score",
        Verb::Return => "return",
        Verb::Transfer => "transfer",
        Verb::Junk => "junk",
        Verb::Tuck => "tuck",
        Verb::Achieve => "achieve",
        Verb::Foreshadow => "foreshadow",
        Verb::Safeguard => "safeguard",
        Verb::Place => "place",
        Verb::Dig => "dig",
        Verb::Rotate => "rotate",
        Verb::Seize => "seize",
    }
}

fn owner(z: Zone) -> Option<Player> {
    match z {
        Zone::Hand(p)
        | Zone::Score(p)
        | Zone::Forecast(p)
        | Zone::Board(p, _)
        | Zone::Achievements(p)
        | Zone::Display(p)
        | Zone::Museum(p)
        | Zone::Safe(p) => Some(p),
        _ => None,
    }
}

fn place(z: Zone, actor: Option<Player>, viewer: usize) -> String {
    let whose = |p| {
        if index(p) == viewer {
            "your"
        } else if Some(p) == actor {
            "their"
        } else {
            "your opponent's"
        }
    };
    match z {
        Zone::Hand(p) => format!("{} hand", whose(p)),
        Zone::Score(p) => format!("{} score pile", whose(p)),
        Zone::Forecast(p) => format!("{} forecast", whose(p)),
        Zone::Board(p, _) => format!("{} board", whose(p)),
        Zone::Achievements(p) => format!("{} achievements", whose(p)),
        Zone::Display(p) => format!("{} display", whose(p)),
        Zone::Museum(p) => format!("{} Museums", whose(p)),
        Zone::Safe(p) => format!("{} safe", whose(p)),
        Zone::AvailableAchievements => "the available achievements".into(),
        Zone::AgeZeroReserve => "the Age 0 reserve".into(),
        Zone::Junk => "the junk".into(),
        Zone::Deck(d) => format!("the {} {} deck", Set::from(d.set).name(), d.age),
    }
}

fn logical_place(z: LogicalZone, viewer: usize) -> String {
    let physical = match z {
        LogicalZone::Hand(p) => Zone::Hand(p),
        LogicalZone::Score(p) => Zone::Score(p),
        LogicalZone::BoardDepth(p, c) => Zone::Board(p, c),
        LogicalZone::AvailableAchievements(_) => Zone::AvailableAchievements,
        LogicalZone::Physical(z) => z,
    };
    place(physical, None, viewer)
}

/// Restrict the chooser's menu again for this viewer. A Known descriptor can
/// name a currently hidden card through native knowledge or a Figure alias.
struct Options<'a> {
    game: &'a Game,
    viewer: usize,
    visible: BTreeSet<FaceId>,
}

impl<'a> Options<'a> {
    fn new(game: &'a Game, viewer: usize, chooser: Player) -> Self {
        let visible = visible_cards(game, viewer)
            .intersection(&visible_cards(game, index(chooser)))
            .copied()
            .collect();
        Self {
            game,
            viewer,
            visible,
        }
    }

    /// Also name cards both players saw revealed ([`revealed_this_action`]).
    fn with_revealed(mut self, revealed: &BTreeSet<CardId>) -> Self {
        let game = self.game;
        self.visible
            .extend(revealed.iter().map(|&card| face_of(game, card)));
        self
    }

    fn card(&self, prefix: &str, card: CardId, hidden: bool) -> OptionView {
        let id = face_of(self.game, card);
        let known = !hidden && self.visible.contains(&id);
        OptionView {
            segs: vec![strong(prefix), if known { Seg::Card(id) } else { back(id) }],
            card: known.then_some(id),
        }
    }

    fn target(&self, prefix: &str, target: &TargetDescriptor) -> OptionView {
        match *target {
            TargetDescriptor::Known { card, source } => {
                let mut option = self.card(prefix, card, false);
                option
                    .segs
                    .push(text(format!(" from {}", place(source, None, self.viewer))));
                option
            }
            TargetDescriptor::Hidden { source, slot, back } => OptionView {
                segs: vec![
                    strong(prefix),
                    Seg::Back(Set::from(back.set), back.age),
                    text(format!(
                        " from {} · #{}",
                        place(source, None, self.viewer),
                        slot + 1
                    )),
                ],
                card: None,
            },
        }
    }

    fn action(&self, action: Action, p: Player) -> OptionView {
        match action {
            Action::Draw => label(format!(
                "Draw (starting at value {})",
                self.game
                    .position()
                    .highest_top_value(p)
                    .max(u8::from(!self.game.position().age_zero_introduced()))
            )),
            Action::Meld { card } => self.card("Meld ", card, false),
            Action::Achieve { card } => self.card("Achieve ", card, true),
            Action::Dogma { card, auspice } => {
                let mut option = self.card("Dogma ", card, false);
                if auspice {
                    option.segs.push(strong(" with Auspice"));
                }
                option
            }
            Action::Decree => label("Issue a Decree"),
        }
    }

    fn alternative(&self, a: &Alternative) -> OptionView {
        match a {
            Alternative::Move { verb: v, target } => {
                self.target(&format!("{} ", title_case(verb(*v))), target)
            }
            Alternative::Select { target } => self.target("Select ", target),
            Alternative::Reveal { target } => self.target("Reveal ", target),
            Alternative::MatchingPair { first, second } => {
                let mut option = self.target("Select ", first);
                let second = self.target("", second);
                option.segs.push(text(" and "));
                option.segs.extend(second.segs);
                option.card = option.card.or(second.card);
                option
            }
            Alternative::BlindSelect { source, back }
            | Alternative::BlindMove { source, back, .. } => {
                let v = if let Alternative::BlindMove { verb: v, .. } = a {
                    title_case(verb(*v))
                } else {
                    "Select".into()
                };
                OptionView {
                    segs: vec![
                        strong(format!("{v} a random ")),
                        Seg::Back(Set::from(back.set), back.age),
                        text(format!(" from {}", place(*source, None, self.viewer))),
                    ],
                    card: None,
                }
            }
            Alternative::Draw { value, set: s } => label(format!(
                "Draw {} (starting at value {value})",
                Set::from(*s).name()
            )),
            Alternative::Splay {
                player,
                color,
                direction,
            } => OptionView {
                segs: if *direction == n::Splay::None {
                    vec![
                        strong("Unsplay "),
                        text(if index(*player) == self.viewer {
                            ""
                        } else {
                            "your opponent's "
                        }),
                        Seg::Hue(*color),
                    ]
                } else {
                    vec![
                        strong("Splay "),
                        text(if index(*player) == self.viewer {
                            ""
                        } else {
                            "your opponent's "
                        }),
                        Seg::Hue(*color),
                        text(format!(" {}", direction.name().to_lowercase())),
                    ]
                },
                card: None,
            },
            Alternative::Value(v) => label(format!("Value {v}")),
            Alternative::Count(n) => label(format!("Exactly {n} in hand")),
            Alternative::Icon(i) => OptionView {
                segs: vec![if *i == Icon::Food {
                    strong("Food")
                } else {
                    Seg::Icon(*i)
                }],
                card: None,
            },
            Alternative::BonusIcon => label("Bonus"),
            Alternative::Colors(mask) => {
                let mut segs = Vec::new();
                for c in COLORS.into_iter().filter(|&c| mask & (1 << c as u8) != 0) {
                    if !segs.is_empty() {
                        segs.push(text(", "));
                    }
                    segs.push(Seg::Hue(c));
                }
                if segs.is_empty() {
                    segs.push(strong("No colors"));
                }
                OptionView { segs, card: None }
            }
            // Defensive fallback; real decisions supply wording via option_label.
            Alternative::Branch(i) => label(format!("Option {}", i + 1)),
            Alternative::Exchange { left, right } => label(format!(
                "Exchange {} with {}",
                logical_place(*left, self.viewer),
                logical_place(*right, self.viewer)
            )),
            Alternative::Deck(d) => label(format!("{} {} deck", Set::from(d.set).name(), d.age)),
            Alternative::NameFigure(n) => {
                let id = by_code(n.code).filter(|id| self.visible.contains(id));
                OptionView {
                    // Naming from the immutable roster is public, not evidence
                    // that this physical card is in the searched deck.
                    segs: vec![
                        strong("Name "),
                        id.map_or_else(|| strong(title_case(n.title)), Seg::Card),
                    ],
                    card: id,
                }
            }
            Alternative::Pass => label("Name no Figure (search fails)"),
            Alternative::Decline => label("Decline"),
            Alternative::Decree(d) => label(format!("Issue the {} Decree", decree(*d))),
            Alternative::JunkSpecial(s) => label(format!("Junk {}", title_case(s.name()))),
            Alternative::RestoreSpecial(s) => label(format!("Restore {}", title_case(s.name()))),
            Alternative::ClaimSpecial(s) => label(format!("Claim {}", title_case(s.name()))),
            Alternative::SelfExecute(c) => self.card("Self-execute ", *c, false),
            Alternative::Showcase { card, auspice } => self.card(
                if *auspice {
                    "Showcase with Auspice "
                } else {
                    "Showcase "
                },
                *card,
                false,
            ),
            Alternative::Karma { source, .. } => self.card("Karma of ", *source, false),
        }
    }
}

fn label(s: impl Into<String>) -> OptionView {
    OptionView {
        segs: vec![strong(s)],
        card: None,
    }
}

fn decision_prompt(options: &[Alternative]) -> String {
    let kind = |a: &Alternative| match a {
        Alternative::Move { verb: v, .. } | Alternative::BlindMove { verb: v, .. } => {
            format!("Choose a card to {}", verb(*v))
        }
        Alternative::Karma { .. } => "Choose which karma applies".into(),
        Alternative::Colors(mask) if mask.count_ones() <= 1 => "Choose a color".into(),
        Alternative::Colors(_) => "Choose colors".into(),
        Alternative::Value(_) => "Choose a value".into(),
        Alternative::Count(_) => "Choose a final count".into(),
        Alternative::Icon(_) | Alternative::BonusIcon => "Choose an icon".into(),
        Alternative::Reveal { .. } => "Choose a card to reveal".into(),
        Alternative::Select { .. } | Alternative::BlindSelect { .. } => "Choose a card".into(),
        Alternative::Splay { .. } => "Choose a color to splay".into(),
        Alternative::MatchingPair { .. } => "Choose a pair of cards".into(),
        Alternative::Draw { .. } => "Choose a draw".into(),
        Alternative::Exchange { .. } => "Choose an exchange".into(),
        Alternative::Deck(_) => "Choose a deck".into(),
        Alternative::NameFigure(_) => "Name a Figure".into(),
        Alternative::Decree(_) => "Choose a Decree to issue".into(),
        Alternative::JunkSpecial(_) => "Choose a special achievement to junk".into(),
        Alternative::RestoreSpecial(_) => "Choose a special achievement to restore".into(),
        Alternative::ClaimSpecial(_) => "Choose a special achievement to claim".into(),
        Alternative::SelfExecute(_) => "Choose a card to self-execute".into(),
        Alternative::Showcase { .. } => "Choose a showcase to dogma".into(),
        Alternative::Branch(_) | Alternative::Pass | Alternative::Decline => {
            "Choose an option".into()
        }
    };
    let mut prompts = options
        .iter()
        .filter(|a| !matches!(a, Alternative::Pass | Alternative::Decline))
        .map(kind);
    let first = prompts.next().unwrap_or_else(|| "Choose an option".into());
    if prompts.all(|prompt| prompt == first) {
        first
    } else {
        "Choose an option".into()
    }
}

fn decision(m: &Match, viewer: usize) -> Option<DecisionView> {
    match m.prompt() {
        Prompt::Turn { player: p, actions } => {
            let options = Options::new(m.game(), viewer, *p);
            Some(DecisionView {
                player: index(*p),
                prompt: vec![strong("Choose an action")],
                options: actions.iter().map(|&a| options.action(a, *p)).collect(),
            })
        }
        Prompt::Decision(d) => Some(effect_decision(
            m.game(),
            d,
            viewer,
            &revealed_this_action(m),
        )),
        Prompt::Over(_) => None,
    }
}

/// Which of the source card's effects is asking, when that tells the player
/// something: its echo, or one of several dogma effects ("2nd effect").
fn effect_label(game: &Game, face: FaceId, effect: Option<u16>) -> Option<String> {
    let effect = effect?;
    if effect == n::Card::ECHO {
        return Some("echo effect".into());
    }
    let dogmas = paragraphs(face, game.subgames())
        .iter()
        .filter(|p| matches!(p.lead, Lead::Icon(_) | Lead::Food))
        .count();
    if dogmas < 2 {
        return None;
    }
    let n = usize::from(effect) + 1;
    let suffix = match n {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    Some(format!("{n}{suffix} effect"))
}

fn effect_decision(
    game: &Game,
    d: &n::Decision,
    viewer: usize,
    revealed: &BTreeSet<CardId>,
) -> DecisionView {
    let options = Options::new(game, viewer, d.player).with_revealed(revealed);
    let mut prompt = Vec::new();
    // Use native capture-time provenance, not the last Dogma or current zone:
    // copied effects and karma may still be running after their source moves.
    if let Some(card) = d.source_card(player(viewer)) {
        let face = face_of(game, card);
        prompt.push(Seg::Card(face));
        if let Some(label) = effect_label(game, face, d.effect(player(viewer))) {
            prompt.extend([text(" · "), text(label)]);
        }
        prompt.push(text(": "));
    }
    prompt.push(strong(decision_prompt(&d.options)));
    DecisionView {
        player: index(d.player),
        prompt,
        options: d
            .options
            .iter()
            .enumerate()
            .map(|(i, a)| {
                d.option_label(i, player(viewer))
                    .map_or_else(|| options.alternative(a), label)
            })
            .collect(),
    }
}

// --------------------------------------------------------------------- log

fn subject(p: Player, viewer: usize, base: &str, third: &str) -> Vec<Seg> {
    vec![
        Seg::Name(index(p)),
        text(format!(
            " {} ",
            if index(p) == viewer { base } else { third }
        )),
    ]
}

fn moved_card(game: &Game, card: CardId, source: Zone, destination: Zone, viewer: usize) -> Seg {
    let id = face_of(game, card);
    if visible_zone(source, viewer) || visible_zone(destination, viewer) {
        Seg::Card(id)
    } else {
        back(id)
    }
}

fn movement_sentence(game: &Game, m: &Movement, viewer: usize) -> Vec<Seg> {
    let base = m.verb.map_or("draw", verb);
    let mut segs = subject(m.executor, viewer, base, &format!("{base}s"));
    segs.push(moved_card(game, m.card, m.source, m.destination, viewer));
    // Routine draws/melds need no redundant zone, nor do returns (always the
    // bottom of the card's own deck). Transfers always name both ends;
    // scoring from the board and foreign-hand melds name their source.
    if m.verb.is_some() && (m.verb == Some(Verb::Transfer) || m.source != Zone::Hand(m.executor)) {
        segs.push(text(format!(
            " from {}",
            place(m.source, Some(m.executor), viewer)
        )));
    }
    if m.verb == Some(Verb::Transfer) || owner(m.destination).is_some_and(|p| p != m.executor) {
        segs.push(text(format!(
            " to {}",
            place(m.destination, Some(m.executor), viewer)
        )));
    }
    segs
}

/// "You would meld Proverb": the operation a karma interrupts, which may
/// never happen. Its card shows only if its source is visible to the viewer.
fn would_sentence(game: &Game, actor: Player, op: n::Operation, viewer: usize) -> Option<Vec<Seg>> {
    let would = |what: &str| {
        subject(
            actor,
            viewer,
            &format!("would {what}"),
            &format!("would {what}"),
        )
    };
    let seen = |card: CardId, zone: Zone| {
        let id = face_of(game, card);
        if visible_zone(zone, viewer) {
            Seg::Card(id)
        } else {
            back(id)
        }
    };
    Some(match op {
        n::Operation::Move {
            verb: v,
            target,
            destination,
        } => {
            let source = target.physical;
            let mut segs = would(verb(v));
            segs.push(seen(target.card, source));
            if v == Verb::Transfer || source != Zone::Hand(actor) {
                segs.push(text(format!(
                    " from {}",
                    place(source, Some(actor), viewer)
                )));
            }
            if v == Verb::Transfer || owner(destination).is_some_and(|p| p != actor) {
                segs.push(text(format!(
                    " to {}",
                    place(destination, Some(actor), viewer)
                )));
            }
            segs
        }
        n::Operation::Claim { target, .. } => {
            let mut segs = would("achieve");
            segs.push(seen(target.card, target.physical));
            segs
        }
        n::Operation::Draw { value, .. } => {
            let article = if matches!(value, 8 | 11 | 18) {
                "an"
            } else {
                "a"
            };
            would(&format!("draw {article} {value}"))
        }
        n::Operation::DrawAction { .. } => would("take a Draw action"),
        n::Operation::Dogma { card, .. } => {
            let mut segs = would("activate");
            segs.push(Seg::Card(face_of(game, card)));
            segs
        }
        n::Operation::ExecuteEffect { .. } => would("execute an effect"),
        n::Operation::IssueDecree { .. } => would("issue a Decree"),
        n::Operation::Splay {
            color, direction, ..
        } => {
            let mut segs = would("splay");
            segs.push(Seg::Hue(color));
            segs.push(text(format!(" {}", direction.name().to_lowercase())));
            segs
        }
        n::Operation::ExchangeAll { .. } => would("exchange"),
        n::Operation::ProposeWin { winner, .. } => {
            subject(winner, viewer, "would win", "would win")
        }
        n::Operation::RequireFade { player } => {
            subject(player, viewer, "would fade a Figure", "would fade a Figure")
        }
        n::Operation::Ineligible { player } => {
            subject(player, viewer, "would not share", "would not share")
        }
        n::Operation::ShareBonus { earned: true, .. } => would("draw for sharing"),
        n::Operation::ShareBonus { earned: false, .. } => would("not draw for sharing"),
        _ => return None,
    })
}

fn sentence(game: &Game, event: &Event, viewer: usize) -> Vec<Seg> {
    match event {
        Event::Moved(m) => movement_sentence(game, m, viewer),
        // The log turns these into headers.
        Event::TurnBegan { .. } => Vec::new(),
        Event::DogmaStarted {
            player,
            card,
            auspice,
            kind,
        } => {
            let mut segs = subject(*player, viewer, "activate", "activates");
            segs.push(Seg::Card(face_of(game, *card)));
            if *auspice {
                segs.push(text(" with Auspice"));
            }
            match kind {
                n::DogmaKind::Promoted => segs.push(text(" (promoted)")),
                n::DogmaKind::Showcased => segs.push(text(" (showcased)")),
                n::DogmaKind::Free => segs.push(text(" (free action)")),
                n::DogmaKind::Action => {}
            }
            segs
        }
        // An announced choice reads as the menu showed it.
        Event::Chose {
            player,
            choice,
            label: wording,
        } => {
            let mut segs = subject(*player, viewer, "choose", "chooses");
            let shown = wording.map_or_else(
                || Options::new(game, viewer, *player).alternative(choice),
                label,
            );
            segs.extend(shown.segs);
            segs
        }
        Event::MuseumClaimed { player, .. } => {
            let mut segs = subject(*player, viewer, "claim", "claims");
            segs.push(text("a Museum"));
            segs
        }
        Event::Revealed {
            card,
            source,
            executor,
            ..
        } => {
            let mut segs = subject(*executor, viewer, "reveal", "reveals");
            segs.extend([
                Seg::Card(face_of(game, *card)),
                text(format!(" from {}", place(*source, Some(*executor), viewer))),
            ]);
            segs
        }
        Event::Splayed {
            player, color, to, ..
        } => {
            let mut segs = if *to == n::Splay::None {
                subject(*player, viewer, "unsplay", "unsplays")
            } else {
                subject(*player, viewer, "splay", "splays")
            };
            segs.push(Seg::Hue(*color));
            if *to != n::Splay::None {
                segs.push(text(format!(" {}", to.name().to_lowercase())));
            }
            segs
        }
        Event::SpecialMoved {
            special: s,
            destination,
            executor,
            ..
        } => {
            let (p, base, third) = match destination {
                SpecialLocation::Owned(p) => (*p, "claim", "claims"),
                SpecialLocation::Available => (*executor, "restore", "restores"),
                SpecialLocation::Junk => (*executor, "junk", "junks"),
            };
            let mut segs = subject(p, viewer, base, third);
            segs.push(strong(title_case(s.name())));
            segs
        }
        Event::DecreeMoved {
            decree: d,
            source,
            destination,
            executor,
            ..
        } => {
            let (p, base, third) = match destination {
                // Native ClaimDecree is the final stage of issuing a Decree.
                DecreeLocation::Owned(p) => (*p, "issue", "issues"),
                DecreeLocation::Supply => (*executor, "return", "returns"),
                DecreeLocation::Junk => (*executor, "junk", "junks"),
            };
            let mut segs = subject(p, viewer, base, third);
            segs.push(strong(format!("the {} Decree", decree(*d))));
            if let DecreeLocation::Owned(from) = source {
                segs.extend([text(" held by "), Seg::Name(index(*from))]);
            }
            segs
        }
        Event::AgeZeroIntroduced { .. } => vec![text("Age 0 is introduced")],
        Event::Exchanged {
            executor,
            movements,
            ..
        } => {
            let mut segs = subject(*executor, viewer, "exchange", "exchanges");
            for (i, m) in movements.iter().enumerate() {
                if i > 0 {
                    segs.push(text("; "));
                }
                segs.push(moved_card(game, m.card, m.source, m.destination, viewer));
                segs.push(text(format!(
                    " from {} to {}",
                    place(m.source, Some(*executor), viewer),
                    place(m.destination, Some(*executor), viewer)
                )));
            }
            segs
        }
        Event::FigureSearch {
            executor,
            deck,
            name,
            found,
        } => {
            let mut segs = subject(*executor, viewer, "name", "names");
            segs.push(strong(
                name.map_or_else(|| "no Figure".into(), |n| title_case(n.title)),
            ));
            segs.push(text(format!(
                " in {}{}",
                place(Zone::Deck(*deck), Some(*executor), viewer),
                if found.is_some() {
                    " (found)"
                } else {
                    " (not found)"
                }
            )));
            segs
        }
        Event::Shuffled { executor, deck } => {
            let mut segs = subject(*executor, viewer, "shuffle", "shuffles");
            segs.push(text(place(Zone::Deck(*deck), Some(*executor), viewer)));
            segs
        }
        Event::SubgameStarted { executor, depth } => {
            let mut segs = subject(*executor, viewer, "start", "starts");
            segs.push(text(if *depth > 1 {
                format!(
                    "a new game inside this one (subgame {depth}): each player draws and melds a 9"
                )
            } else {
                "a new game, setting every card aside: each player draws and melds a 9".to_string()
            }));
            segs
        }
        Event::SubgameEnded { winner, depth } => match winner {
            Some(winner) => {
                let mut segs = vec![text(format!("Subgame {depth} over: "))];
                segs.push(Seg::Name(index(*winner)));
                segs.push(text(if index(*winner) == viewer {
                    " win it and carry your cards into the game it interrupted"
                } else {
                    " wins it and carries their cards into the game it interrupted"
                }));
                segs
            }
            None => vec![text(format!(
                "Subgame {depth} over: a draw, so every card from it is junked"
            ))],
        },
        Event::Transferred {
            card,
            source,
            destination,
            owner,
        } => {
            let mut segs = vec![moved_card(game, *card, *source, *destination, viewer)];
            segs.push(text(if *destination == Zone::Junk {
                format!(" from {} is junked", place(*source, Some(*owner), viewer))
            } else {
                format!(" joins {}", place(*destination, Some(*owner), viewer))
            }));
            segs
        }
        Event::Flipped { executor, won } => {
            let mut segs = subject(*executor, viewer, "flip", "flips");
            segs.push(text(if *won {
                " a coin: a win"
            } else {
                " a coin: a loss"
            }));
            segs
        }
        // The Figure is a public top card; its effects follow indented.
        Event::KarmaTriggered {
            card,
            reaction,
            actor,
            would,
            ..
        } => {
            let figure = Seg::Card(face_of(game, *card));
            let lead = match reaction {
                n::Reaction::First => ", but first (",
                n::Reaction::Instead => ", but instead (",
                n::Reaction::Melded => "",
            };
            match would.and_then(|op| would_sentence(game, *actor, op, viewer)) {
                Some(mut segs) if !lead.is_empty() => {
                    if let Some(Seg::Text(last)) = segs.last_mut() {
                        last.truncate(last.trim_end().len());
                    }
                    segs.extend([text(lead), figure, text(" karma)")]);
                    segs
                }
                _ => vec![figure, text(" karma triggers")],
            }
        }
        Event::Endorsed { player, card } => {
            let mut segs = subject(*player, viewer, "endorse", "endorses");
            segs.extend([Seg::Card(face_of(game, *card)), text(" (Cities)")]);
            segs
        }
        // "You self-execute Computers [9] (Yeager's Bell X-1A [9] effect)":
        // the executed card is public, and so is the card that asked for it
        // (a Figure: its karma). Its effects follow indented.
        Event::ExecutionStarted {
            executor,
            card,
            mode,
            source,
            ..
        } => {
            let (base, third) = match mode {
                n::ExecutionMode::SelfExecute => ("self-execute", "self-executes"),
                n::ExecutionMode::SuperExecute => ("super-execute", "super-executes"),
                n::ExecutionMode::Effect(_) => ("execute", "executes"),
            };
            let mut segs = subject(*executor, viewer, base, third);
            if let n::ExecutionMode::Effect(index) = mode {
                segs.push(text(format!("effect {} of ", index + 1)));
            }
            segs.push(Seg::Card(face_of(game, *card)));
            if let Some(source) = source {
                let cause = if game.position().card(*source).figure.is_some() {
                    " karma)"
                } else {
                    " effect)"
                };
                segs.extend([text(" ("), Seg::Card(face_of(game, *source)), text(cause)]);
            }
            segs
        }
        // Only closes the indentation its start opened.
        Event::ExecutionEnded { .. } => Vec::new(),
        // "You first draw and achieve an 11 (Chain rule)": the rule behind
        // the draw and achieve that follow a self- or super-execution inside
        // a card effect.
        Event::ChainAwarded { executor, .. } => {
            let mut segs = subject(*executor, viewer, "first draw", "first draws");
            segs.push(text("and achieve an 11 (Chain rule)"));
            segs
        }
    }
}

/// How deep the log nests the lines of one game clock: the effects of a
/// Dogma action and of each open execution sit one level in, and a karma
/// body one level further. The engine flags each fact inside a karma; a
/// choice carries no flag and follows the last fact's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Nesting {
    depth: u8,
    karma: bool,
}

impl Nesting {
    /// A regular action resets the nesting; a Dogma's effects nest under it.
    fn action(&mut self, dogma: bool) {
        *self = Self {
            depth: u8::from(dogma),
            karma: false,
        };
    }

    /// The level of this event's line, opening or closing what it nests.
    fn level(&mut self, event: &Event) -> u8 {
        let karma = match event {
            Event::Moved(m) => Some(m.inside_karma),
            Event::DecreeMoved { inside_karma, .. }
            | Event::SpecialMoved { inside_karma, .. }
            | Event::Revealed { inside_karma, .. }
            | Event::MuseumClaimed { inside_karma, .. }
            | Event::Splayed { inside_karma, .. }
            | Event::Exchanged { inside_karma, .. }
            | Event::ExecutionStarted { inside_karma, .. } => Some(*inside_karma),
            // These facts lack a flag, but in a Base/Figures Match their only
            // producers are Bear Grylls (Age 0) and Alex Trebek (search/shuffle).
            // Unlike arbitrary Game fixtures, both are necessarily karma here.
            Event::AgeZeroIntroduced { .. }
            | Event::FigureSearch { .. }
            | Event::Shuffled { .. } => Some(true),
            // Endorsement is a rule of the Dogma action, never karma; a coin
            // flip is Quantum Computers' own operation.
            Event::Endorsed { .. }
            | Event::Flipped { .. }
            | Event::SubgameStarted { .. }
            | Event::SubgameEnded { .. }
            | Event::Transferred { .. }
            | Event::TurnBegan { .. }
            | Event::DogmaStarted { .. }
            | Event::Chose { .. }
            | Event::ExecutionEnded { .. }
            | Event::ChainAwarded { .. } => None,
            // A karma body never nests another: its line sits at the depth.
            Event::KarmaTriggered { .. } => {
                self.karma = true;
                return self.depth;
            }
        };
        if let Some(karma) = karma {
            self.karma = karma;
        }
        match event {
            Event::ExecutionStarted { .. } | Event::DogmaStarted { .. } => {
                let level = self.depth + u8::from(self.karma);
                self.depth = self.depth.saturating_add(1);
                level
            }
            Event::ExecutionEnded { .. } => {
                self.depth = self.depth.saturating_sub(1);
                self.depth
            }
            _ => self.depth + u8::from(self.karma),
        }
    }
}

fn line(out: &mut Vec<Entry>, level: u8, segs: Vec<Seg>) {
    if let Some(Entry::Line(previous_level, previous, count)) = out.last_mut()
        && *previous_level == level
        && *previous == segs
    {
        *count += 1;
    } else {
        out.push(Entry::Line(level, segs, 1));
    }
}

/// The turn clock of one game: the main game or a Martian Internet subgame.
struct Level {
    turn: usize,
    owner: Player,
    actions: usize,
    nesting: Nesting,
}

/// "Turn 7: You", or "Subgame 1 · Turn 2: AI" inside a subgame.
fn turn_header(depth: usize, level: &Level, viewer: usize, two_player: bool) -> String {
    let owner = name(index(level.owner), viewer, two_player);
    if depth == 0 {
        format!("Turn {}: {owner}", level.turn)
    } else {
        format!("Subgame {depth} · Turn {}: {owner}", level.turn)
    }
}

fn log_entries(m: &Match, viewer: usize, two_player: bool) -> Vec<Entry> {
    let mut out = vec![Entry::Header("Setup".into(), false)];
    let mut events = m.events().iter().peekable();
    // Headers follow the engine's turn starts, so a turn's start procedure
    // (an Artifacts showcase) sits under its own turn. A subgame keeps its
    // own clock; the game it interrupted resumes its turn when it ends.
    let mut levels = vec![Level {
        turn: 0,
        owner: m.game().position().turn_owner(),
        actions: 0,
        nesting: Nesting::default(),
    }];
    // Iterate inputs as well as events: an action may emit no facts at all,
    // and all decisions inside an action share its turn/action grouping.
    for tag in 0..=m.record().steps.len() {
        if let Some(Step::Action(action)) = tag.checked_sub(1).and_then(|i| m.record().steps.get(i))
        {
            let level = levels.last_mut().expect("the main game");
            if level.actions > 0 {
                out.push(Entry::Gap);
            }
            level.actions += 1;
            level.nesting.action(matches!(action, Action::Dogma { .. }));
            match action {
                Action::Dogma { card, auspice } => {
                    // A legal Dogma source is a public board top at this input,
                    // regardless of where that card has moved since then.
                    let mut segs = subject(level.owner, viewer, "activate", "activates");
                    segs.push(Seg::Card(face_of(m.game(), *card)));
                    if *auspice {
                        segs.push(text(" with Auspice"));
                    }
                    line(&mut out, 0, segs);
                }
                Action::Decree => {
                    let mut segs = subject(level.owner, viewer, "choose", "chooses");
                    segs.push(text("to issue a Decree"));
                    line(&mut out, 0, segs);
                }
                _ => {}
            }
        }
        while events.peek().is_some_and(|(step, _)| *step == tag) {
            let (_, event) = events.next().expect("peeked event");
            match event {
                Event::TurnBegan { player } => {
                    let depth = levels.len() - 1;
                    let level = levels.last_mut().expect("the main game");
                    level.turn += 1;
                    level.owner = *player;
                    level.actions = 0;
                    level.nesting = Nesting::default();
                    out.push(Entry::Header(
                        turn_header(depth, level, viewer, two_player),
                        false,
                    ));
                }
                // The subgame starts inside Martian Internet's effect and
                // keeps its own clock; its end returns to that effect.
                Event::SubgameStarted { .. } => {
                    let level = levels.last_mut().expect("the main game");
                    let at = level.nesting.level(event);
                    line(&mut out, at, sentence(m.game(), event, viewer));
                    let owner = level.owner;
                    levels.push(Level {
                        turn: 0,
                        owner,
                        actions: 0,
                        nesting: Nesting::default(),
                    });
                }
                Event::SubgameEnded { .. } => {
                    if levels.len() > 1 {
                        levels.pop();
                    }
                    let depth = levels.len() - 1;
                    let level = levels.last_mut().expect("the main game");
                    let at = level.nesting.level(event);
                    line(&mut out, at, sentence(m.game(), event, viewer));
                    out.push(Entry::Header(
                        format!(
                            "{} (continued)",
                            turn_header(depth, level, viewer, two_player)
                        ),
                        false,
                    ));
                }
                // The action's input already named it, even if a karma
                // replaced the Dogma.
                Event::DogmaStarted {
                    kind: n::DogmaKind::Action,
                    ..
                } => {}
                _ => {
                    let level = levels.last_mut().expect("the main game");
                    let at = level.nesting.level(event);
                    if !matches!(event, Event::ExecutionEnded { .. }) {
                        line(&mut out, at, sentence(m.game(), event, viewer));
                    }
                }
            }
        }
    }
    if matches!(m.prompt(), Prompt::Over(_)) {
        out.push(Entry::Header("Game over".into(), false));
    }
    if let Some(Entry::Header(_, current)) = out
        .iter_mut()
        .rev()
        .find(|e| matches!(e, Entry::Header(..)))
    {
        *current = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::table::plain;
    use super::*;

    /// Every engine-next deal the app offers: each of the sixteen Artifacts,
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
                                    .filter(
                                        |&s| pos.special_location(s) == SpecialLocation::Owned(p)
                                    )
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

    fn assert_private_log(
        m: &Match,
        initial: &BTreeMap<CardId, Zone>,
        viewer: usize,
        log: &[Entry],
    ) {
        let mut physical = initial.clone();
        let mut events = m.events().iter().peekable();
        let mut allowed = Vec::<BTreeSet<FaceId>>::new();
        for tag in 0..=m.record().steps.len() {
            if let Some(Step::Action(a)) = tag.checked_sub(1).and_then(|i| m.record().steps.get(i))
            {
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
                let mut moved = |c: CardId, src: Zone, dst: Zone| {
                    assert_eq!(physical[&c], src);
                    if public_to(m.game(), src, viewer) || public_to(m.game(), dst, viewer) {
                        visible.insert(face_of(m.game(), c));
                    }
                    physical.insert(c, dst);
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
                for viewer in 0..2 {
                    assert_private_log(&m, &initial, viewer, &log_entries(&m, viewer, true));
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
        let cards = ["B003", "B001", "B006", "B004"].map(|code| n::base_card(code).unwrap());
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

    #[test]
    fn propaganda_and_ninja_log_the_demanded_color() {
        for (code, color) in [("U024", n::Color::Purple), ("U036", n::Color::Red)] {
            let cards = vec![n::unseen_card(code).unwrap(), n::base_card("B001").unwrap()];
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
            n::unseen_card("U040").unwrap(),
            n::base_card("B001").unwrap(),
            n::base_card("B002").unwrap(),
            n::base_card("B003").unwrap(),
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
            n::unseen_card("U040").unwrap(),
            n::figure_card("F110").unwrap(),
            n::base_card("B001").unwrap(),
        ];
        cards.extend(["Z001", "Z002"].map(|code| n::age_zero_card(code).unwrap()));
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
            n::base_card("B020").unwrap(),
            n::echoes_card("E013").unwrap(),
            n::echoes_card("E001").unwrap(),
            n::echoes_card("E002").unwrap(),
            n::base_card("B003").unwrap(),
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
        let promoted = events.iter().find(|e| matches!(e,
            Event::Moved(m) if m.card == CardId(1) && m.source == Zone::Forecast(Player::One)
        )).unwrap();
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
                    n::city_card("C040").unwrap(), // Calicut: Search for Health.
                    n::base_card("B040").unwrap(), // Existing green pile.
                    n::base_card("B039").unwrap(), // Perspective: outside the packet.
                    n::base_card("B036").unwrap(), // Colonialism: return.
                    n::base_card("B037").unwrap(), // Gunpowder: return.
                    n::base_card("B038").unwrap(), // Anatomy: take into hand.
                    n::base_card("B041").unwrap(), // Navigation: return.
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
                    for &card in game.position().cards(deck) {
                        assert!(!visible_cards(&game, viewer).contains(&face_of(&game, card)));
                    }
                }
            }
        }
    }

    fn fixture() -> Game {
        let cards = vec![
            n::base_card("B006").unwrap(),
            n::base_card("B019").unwrap(),
            n::figure_card("F001").unwrap(),
            n::base_card("B012").unwrap(),
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
        assert!(plain(&sentence, 0, false).starts_with(
            "You exchange Masonry [1] from your hand to your opponent's score pile; "
        ));
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
            n::figure_card("F001").unwrap(),
            n::figure_card("F002").unwrap(),
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
            n::figure_card("F002").unwrap(),
            n::base_card("B002").unwrap(),
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
        let tools = n::base_card("B011").unwrap();
        let color = tools.color;
        let mut cards = vec![tools];
        cards.extend(["B001", "B002", "B003"].map(|code| n::base_card(code).unwrap()));
        cards.push(n::base_card("B031").unwrap()); // an age-3 card for the hand
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
        let newton = n::figure_card("F052").unwrap();
        let color = newton.color;
        let cards = vec![newton, n::base_card("B004").unwrap()];
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
        let mut card = n::base_card("B018").unwrap();
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
        let mut game = Game::new_base(1).unwrap();
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
            n::figure_card("F086").unwrap(), // Mandela, red
            n::figure_card("F005").unwrap(), // Shennong, yellow
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
}
