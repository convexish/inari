//! A static check of the proof-reveal convention: a move that selects a hidden card by
//! its front and keeps it hidden must reveal it, and a mandatory single such move must
//! prove an empty selection. The privacy sampler cannot catch these omissions, since
//! the chooser sees their own cards. Every exception carries its reason.
use super::*;
use crate::model::Verb;
use crate::program::{
    Alias, Amount, ColorSet, Cond, Destination, Expr, Filter, Instruction, Selector, Slot, Stop,
    Test, Who,
};

fn hidden_source(selector: &Selector) -> bool {
    match selector {
        Selector::Hand(_) | Selector::Score(_) | Selector::Forecast(_) | Selector::Safe(_) => true,
        Selector::Union(parts) => parts.iter().any(hidden_source),
        Selector::Filtered(source, _) => hidden_source(source),
        _ => false,
    }
}

fn front_filtered(selector: &Selector) -> bool {
    match selector {
        Selector::Filtered(source, filters) => {
            filters.iter().any(|filter| {
                matches!(filter, Filter::Has(test) | Filter::Lacks(test) if test.reads_front())
            }) || front_filtered(source)
        }
        Selector::Union(parts) => parts.iter().any(front_filtered),
        _ => false,
    }
}

/// Destinations the other player sees: a board, or the demander's hand or
/// score pile when the executor transfers to it.
fn public_destination(verb: Verb, to: Destination) -> bool {
    matches!(to, Destination::Board(_) | Destination::Origin(_))
        || matches!(verb, Verb::Meld | Verb::Tuck)
        || matches!(
            to,
            Destination::Hand(Who::Demander | Who::Opponent)
                | Destination::Score(Who::Demander | Who::Opponent)
        )
}

/// Moves whose selected cards an earlier explicit reveal already shows.
const REVEALED_FIRST: &[(&str, &str)] = &[
    (
        "B060 #0",
        "Classification reveals both hands before its sweeps",
    ),
    ("E007 #0", "Comb returns only its revealed draws"),
    (
        "E110 #0",
        "Algocracy reveals a frozen packet before transferring",
    ),
    ("A013 #0", "Mask of Warka reveals both hands first"),
    ("A039 #0", "Mona Lisa reveals the hand first"),
    ("Z004 #0", "Fishing reveals all hands first"),
];

/// Mandatory single moves whose failure an explicit reveal already proves.
const PROVED_FIRST: &[(&str, &str)] = &[
    ("E002 #0", "Candles reveals the hand when nothing qualifies"),
    ("E055 #0", "Tuning Fork melds only its revealed draw"),
    ("Z014 #0", "First Aid reveals the hand first"),
    (
        "U002 #0",
        "Maze reveals unshown hand cards after a failed score",
    ),
];

fn programs() -> Vec<(String, &'static [Instruction])> {
    let mut all = Vec::new();
    let mut card = |card: Card| {
        if let Some(echo) = &card.echo {
            all.push((
                format!("{} echo", card.code),
                echo.instructions.clone().leak() as &[_],
            ));
        }
        for (index, effect) in card.effects.iter().enumerate() {
            all.push((
                format!("{} #{index}", card.code),
                effect.instructions.clone().leak() as &[_],
            ));
        }
        if let Some(figure) = card.figure {
            for (index, karma) in figure.def().karma.iter().enumerate() {
                if let crate::program::Body::Program(program) = karma.body {
                    all.push((format!("{} karma {index}", card.code), program));
                }
            }
        }
    };
    for m in BASE_METADATA {
        card(base_card(m.code).unwrap());
    }
    for m in ECHOES_METADATA {
        card(echoes_card(m.code).unwrap());
    }
    for m in ARTIFACTS_METADATA {
        card(artifacts_card(m.code).unwrap());
    }
    for m in UNSEEN_METADATA {
        card(unseen_card(m.code).unwrap());
    }
    for m in FIGURE_METADATA {
        card(figure_card(m.code).unwrap());
    }
    for m in AGE_ZERO_METADATA {
        card(age_zero_card(m.code).unwrap());
    }
    all
}

#[test]
fn front_filtered_hidden_moves_carry_proofs() {
    let mut unverified = Vec::new();
    let mut unproved = Vec::new();
    for (name, program) in programs() {
        for instruction in program {
            let Instruction::Move(step) = instruction else {
                continue;
            };
            if !hidden_source(&step.from) || !front_filtered(&step.from) {
                continue;
            }
            if !step.verify && !public_destination(step.verb, step.to) {
                unverified.push(name.clone());
            }
            if matches!(step.amount, Amount::One { optional: false })
                && step.reveal_if_empty.is_none()
            {
                unproved.push(name.clone());
            }
        }
    }
    unverified.retain(|name| !REVEALED_FIRST.iter().any(|(code, _)| code == name));
    unproved.retain(|name| !PROVED_FIRST.iter().any(|(code, _)| code == name));
    assert!(
        unverified.is_empty(),
        "front-filtered hidden moves without a reveal: {unverified:?}"
    );
    assert!(
        unproved.is_empty(),
        "mandatory front-filtered moves without an empty proof: {unproved:?}"
    );
}

/// The hands a selector reads, except parts limited to cards an earlier
/// reveal recorded in one of `shown`.
fn hands(selector: &Selector, shown: &[Slot], out: &mut Vec<Who>) {
    match selector {
        Selector::Hand(who) => out.push(*who),
        Selector::Union(parts) => parts.iter().for_each(|part| hands(part, shown, out)),
        Selector::Filtered(source, filters) => {
            let recorded = filters.iter().any(
                |filter| matches!(filter, Filter::Has(Test::InSlot(slot)) if shown.contains(slot)),
            );
            if !recorded {
                hands(source, shown, out);
            }
        }
        _ => {}
    }
}

/// Whether a selector reads a slot, such as a reveal's record of its cards.
fn reads_slot(selector: &Selector) -> bool {
    match selector {
        Selector::Union(parts) => parts.iter().any(reads_slot),
        Selector::Filtered(source, filters) => {
            filters.iter().any(|filter| {
                matches!(
                    filter,
                    Filter::Has(Test::InSlot(_)) | Filter::Lacks(Test::InSlot(_))
                )
            }) || reads_slot(source)
        }
        _ => false,
    }
}

/// The slots earlier reveals recorded into.
fn revealed_slots(earlier: &[Instruction]) -> Vec<Slot> {
    earlier
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::Reveal {
                keep: Some(keep), ..
            } => Some(keep.slot),
            _ => None,
        })
        .collect()
}

/// Whether an earlier full reveal shows `who`'s hand.
fn revealed(earlier: &[Instruction], who: Who) -> bool {
    earlier.iter().any(|instruction| {
        let Instruction::Reveal {
            from, all: true, ..
        } = instruction
        else {
            return false;
        };
        let mut shown = Vec::new();
        hands(from, &[], &mut shown);
        let covers = |player: Who| shown.contains(&player) || shown.contains(&Who::Either);
        match who {
            Who::Either => covers(Who::Executor) && covers(Who::Opponent),
            player => covers(player),
        }
    })
}

/// Hand sweeps over cards already shown some other way.
const SHOWN_OTHERWISE: &[(&str, &str)] = &[(
    "E007 #0",
    "Comb returns only its draws, each revealed as drawn",
)];

/// Effects that act on every hand card matching a front test reveal that hand before
/// anything moves. The player then orders the matches as known cards, which matters
/// when a karma reacts to each one. A hand the mover cannot see is read through the
/// reveal's record, never blind.
#[test]
fn front_filtered_hand_sweeps_reveal_the_hand_first() {
    let mut late = Vec::new();
    for (name, program) in programs() {
        if SHOWN_OTHERWISE.iter().any(|(code, _)| *code == name) {
            continue;
        }
        for (index, instruction) in program.iter().enumerate() {
            let Instruction::Move(step) = instruction else {
                continue;
            };
            let Amount::Batch {
                limit: None,
                stop: Stop::Never | Stop::Initially,
                per_color: false,
                ..
            } = step.amount
            else {
                continue;
            };
            if !front_filtered(&step.from) {
                continue;
            }
            let earlier = &program[..index];
            let mut read = Vec::new();
            hands(&step.from, &revealed_slots(earlier), &mut read);
            let unrevealed = read.iter().any(|&who| !revealed(earlier, who));
            let blind = read.iter().any(|&who| who != Who::Executor) && !reads_slot(&step.from);
            if unrevealed || blind {
                late.push(format!("{name} @{index}"));
            }
        }
    }
    assert!(
        late.is_empty(),
        "hand sweeps by front without a prior reveal: {late:?}"
    );
}

/// A selector whose cards, counts and printed values both players can read:
/// backs show values and sets, so hidden piles are public unless a filter
/// reads their fronts. Registers and slots are left out (their cards may be
/// hidden draws); no alias needs them.
fn public_selector(selector: &Selector) -> bool {
    if hidden_source(selector) && front_filtered(selector) {
        return false;
    }
    match selector {
        Selector::Union(parts) => parts.iter().all(public_selector),
        Selector::Filtered(source, filters) => {
            public_selector(source) && filters.iter().all(public_filter)
        }
        Selector::Board(_, colors, _) => public_colors(colors),
        Selector::BaseDeck(age) | Selector::SetDeck(_, age) | Selector::BaseDeckTop(age) => {
            public_expr(age)
        }
        Selector::Reg(_) | Selector::Slot(_) => false,
        _ => true,
    }
}

fn public_filter(filter: &Filter) -> bool {
    match filter {
        Filter::Has(test) | Filter::Lacks(test) => public_test(test),
        Filter::Lowest | Filter::Highest => true,
    }
}

/// A test's own operands are public; whether the tested card's front may be
/// read is `public_selector`'s question.
fn public_test(test: &Test) -> bool {
    match test {
        Test::Value(_, expr) => public_expr(expr),
        Test::ValueIn(source)
        | Test::SharesIconWith(source)
        | Test::ValueRepeated(source)
        | Test::ValueCount(source, ..) => public_selector(source),
        Test::ColorIn(colors) => public_colors(colors),
        Test::CardsOfColorOnBoard(_, _, expr) => public_expr(expr),
        _ => true,
    }
}

fn public_colors(colors: &ColorSet) -> bool {
    match colors {
        ColorSet::Only(_)
        | ColorSet::Chosen
        | ColorSet::MostCards(_)
        | ColorSet::MostVisible(_) => true,
        ColorSet::OfCards(source) => public_selector(source) && !hidden_source(source),
        ColorSet::Of(_) | ColorSet::Slot(_) => false,
    }
}

fn public_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Const(_)
        | Expr::Boxed(_)
        | Expr::Chosen
        | Expr::Metric(..)
        | Expr::MostVisibleInAColor(_)
        | Expr::Bonuses(..)
        | Expr::Value(_) => true,
        Expr::Count(source)
        | Expr::Max(source)
        | Expr::Min(source)
        | Expr::Distinct(source)
        | Expr::Total(source) => public_selector(source),
        Expr::DistinctColors(source)
        | Expr::IconsAmong(source, _)
        | Expr::IconTypesAmong(source) => public_selector(source) && !hidden_source(source),
        Expr::Add(inner, _) | Expr::Mul(inner, _) | Expr::Div(inner, _) | Expr::NonZero(inner) => {
            public_expr(inner)
        }
        Expr::Sum(terms) => terms.iter().all(public_expr),
        Expr::Visible(query) => public_colors(&query.colors),
        Expr::IconsOn(..)
        | Expr::BonusOf(_)
        | Expr::Slot(_)
        | Expr::Done
        | Expr::DoneValues
        | Expr::DemandMoved(_)
        | Expr::ThisTurn(_) => false,
    }
}

fn public_cond(cond: &Cond) -> bool {
    match cond {
        Cond::Compare(a, _, b) | Cond::SameDraw(a, b) => public_expr(a) && public_expr(b),
        Cond::CommonIcon { from, .. } => public_selector(from) && !hidden_source(from),
        Cond::EqualValues(a, b) => public_selector(a) && public_selector(b),
        Cond::All(conds) | Cond::Any(conds) => conds.iter().all(public_cond),
        Cond::Not(inner) => public_cond(inner),
        Cond::Splayed { colors, .. } => public_colors(colors),
        Cond::Action(_)
        | Cond::Foreseen
        | Cond::OwnTurn
        | Cond::Must
        | Cond::DancingGirlVisited => true,
        Cond::Card(..)
        | Cond::Empty(_)
        | Cond::Did(_)
        | Cond::FinalCount { .. }
        | Cond::DemandMoved(_)
        | Cond::DrawnColor(_)
        | Cond::DrawnColorTwice
        | Cond::ReturnedMost => false,
    }
}

/// An alias merges menu options, so its condition or key must read only what
/// both players see: a merged menu's shape would otherwise disclose a hidden
/// front. The interpreter applies aliases without consulting a tracker, so
/// tracked games and untracked search worlds agree; this lint is the privacy
/// check.
#[test]
fn alias_conditions_are_public() {
    let mut private = Vec::new();
    let mut annotated = 0;
    for (name, program) in programs() {
        for instruction in program {
            let Instruction::Choose {
                alias: Some(alias), ..
            } = instruction
            else {
                continue;
            };
            annotated += 1;
            let public = match alias {
                Alias::NoOpWhen(cond) => public_cond(cond),
                Alias::SameKey(expr) => public_expr(expr),
            };
            if !public {
                private.push(format!("{name}: {alias:?}"));
            }
        }
    }
    assert!(annotated >= 20, "{annotated} aliases found");
    assert!(
        private.is_empty(),
        "aliases over hidden fronts: {private:#?}"
    );
}
