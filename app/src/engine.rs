//! The engine's side of the table, for every expansion combination: the view
//! snapshot, decision options as match steps, and its log.
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

/// The face of an engine card, by publisher code.
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
    view_with_log(m, viewer, two_player, peek, true)
}

fn view_with_log(
    m: &Match,
    viewer: usize,
    two_player: bool,
    peek: bool,
    include_log: bool,
) -> TableView {
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
        log: if include_log {
            log_entries(m, viewer, two_player)
        } else {
            Vec::new()
        },
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
    // Museums are unnumbered in the engine: stored ones count from one,
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
    segs.push(if m.public {
        Seg::Card(face_of(game, m.card))
    } else {
        moved_card(game, m.card, m.source, m.destination, viewer)
    });
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
mod tests;
