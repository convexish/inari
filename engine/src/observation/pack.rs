//! Fixed-capacity batch packing of next-1 rows for Torch (train/next_inference.py
//! `collate_next1`). One flat `f32` buffer, uploaded with a single host-to-device
//! copy, holds in order:
//!
//! | Part | Floats | Layout |
//! |---|---:|---|
//! | Scalars | `rows × SCALARS` | row-major |
//! | Seats | `rows` | header seat per row |
//! | Pooled tokens | `6 × tokens` | planes row, card, group, weight, code, value; every token whose group is not Context, in row order |
//! | Records | `records × 56` | row, card, kind, depth, present, 0, then field ids 1..=50 recombined from their limbs |
//! | Options | `options × 15` | row, present, then the 13 option fields |
//!
//! Padding is zero: a padded token has weight 0, a padded record or option
//! has `present` 0. Record `depth` is the length of the Parent chain within
//! the row, clamped to [`MAX_DEPTH`]. Every categorical value is validated as
//! an in-range integer so a corrupt row fails here instead of indexing an
//! embedding table.
//!
//! The packer reads and writes through the [`Source`] and [`Sink`] traits so
//! the Python module can pack straight from and into buffer-protocol memory
//! without copying either side.
use std::cell::Cell;

use super::{
    FAMILY, GENERATION, GROUPS, Group, HEADER, OPTION_FIELDS, REVISION, SCALARS, TOKEN_FIELDS,
    context,
};
use crate::STABLE_IDS;

/// Pooled token planes: row, card, group, weight, code, value.
pub const TOKEN_PLANES: usize = 6;
/// Record layout: row, card, kind, depth, present, reserved, then the fields.
pub const RECORD_WIDTH: usize = 6 + FIELDS;
/// Field id `i` (1-based) lives at `RECORD_FIELD_BASE + i` within a record.
pub const RECORD_FIELD_BASE: usize = 5;
/// Option layout: row, present, then the option fields.
pub const OPTION_WIDTH: usize = 2 + OPTION_FIELDS;
/// Context field ids are 1..=FIELDS.
pub const FIELDS: usize = context::Field::PublicOutcome as usize;
/// Record kinds are 1..=KINDS.
pub const KINDS: usize = context::RecordKind::Opaque as usize;
/// Card identities are stable ids plus one.
pub const CARDS: usize = STABLE_IDS as usize + 1;
/// Pooled token codes stay below this bound.
pub const POOLED_CODES: u32 = 4096;
/// Recombined field values are clamped so they stay exact in `f32`.
pub const MAX_FIELD_VALUE: u64 = (1 << 24) - 1;
pub const MAX_DEPTH: u8 = 7;

const CONTEXT: u32 = Group::Context as u32;
const LIMB: u64 = 1 << 16;

/// Row, pooled-token, record and option counts: a batch's measured sizes or a
/// buffer's capacities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub rows: usize,
    pub tokens: usize,
    pub records: usize,
    pub options: usize,
}

impl Counts {
    /// Floats in a buffer with these capacities.
    pub fn width(self) -> usize {
        self.rows * (SCALARS + 1)
            + TOKEN_PLANES * self.tokens
            + RECORD_WIDTH * self.records
            + OPTION_WIDTH * self.options
    }
}

/// Read access to the concatenated rows.
pub trait Source {
    fn len(&self) -> usize;
    fn at(&self, index: usize) -> f32;
    /// `N` consecutive floats from `index`; implementors can bounds-check once.
    #[inline]
    fn chunk<const N: usize>(&self, index: usize) -> [f32; N] {
        std::array::from_fn(|i| self.at(index + i))
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Source for &[f32] {
    #[inline]
    fn len(&self) -> usize {
        <[f32]>::len(self)
    }
    #[inline]
    fn at(&self, index: usize) -> f32 {
        self[index]
    }
    #[inline]
    fn chunk<const N: usize>(&self, index: usize) -> [f32; N] {
        self[index..index + N].try_into().expect("exact length")
    }
}

/// Write access to the packed buffer.
pub trait Sink {
    fn len(&self) -> usize;
    fn set(&mut self, index: usize, value: f32);
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Sink for &mut [f32] {
    #[inline]
    fn len(&self) -> usize {
        <[f32]>::len(self)
    }
    #[inline]
    fn set(&mut self, index: usize, value: f32) {
        self[index] = value;
    }
}

impl Sink for &[Cell<f32>] {
    #[inline]
    fn len(&self) -> usize {
        <[Cell<f32>]>::len(self)
    }
    #[inline]
    fn set(&mut self, index: usize, value: f32) {
        self[index].set(value);
    }
}

/// Counts a batch without writing it (unbounded capacities).
struct Measure;

impl Sink for Measure {
    fn len(&self) -> usize {
        usize::MAX
    }
    fn set(&mut self, _: usize, _: f32) {}
}

/// Validate `sizes` consecutive rows of `flat` and count what packing them
/// needs, so the caller can pick capacities before allocating.
pub fn measure<S: Source>(flat: S, sizes: &[usize]) -> Result<Counts, String> {
    Walker {
        at: Regions::of(Counts::default()),
        cap: None,
        out: Measure,
        used: Counts::default(),
        depths: Vec::new(),
    }
    .walk(flat, sizes)
}

/// Pack `sizes` consecutive rows of `flat` into `out`, a buffer of exactly
/// `capacity.width()` floats; returns the used counts. On an error `out` is
/// partially written.
pub fn pack<S: Source, O: Sink>(
    flat: S,
    sizes: &[usize],
    capacity: Counts,
    out: O,
) -> Result<Counts, String> {
    if out.len() != capacity.width() {
        return Err(format!(
            "packed buffer holds {} floats but the capacities need {}",
            out.len(),
            capacity.width()
        ));
    }
    let mut walker = Walker {
        at: Regions::of(capacity),
        cap: Some(capacity),
        out,
        used: Counts::default(),
        depths: Vec::new(),
    };
    let counts = walker.walk(flat, sizes)?;
    walker.zero_padding(capacity);
    Ok(counts)
}

/// Offsets of the parts of a buffer with capacities `cap`.
struct Regions {
    seats: usize,
    tokens: usize,
    records: usize,
    options: usize,
}

impl Regions {
    fn of(cap: Counts) -> Self {
        let seats = cap.rows * SCALARS;
        let tokens = seats + cap.rows;
        let records = tokens + TOKEN_PLANES * cap.tokens;
        Self {
            seats,
            tokens,
            records,
            options: records + RECORD_WIDTH * cap.records,
        }
    }
}

/// A nonnegative integer below `bound`, exact in `f32`.
#[inline]
fn index(value: f32, bound: usize) -> Option<usize> {
    if (0.0..=16_777_216.0).contains(&value) && value.fract() == 0.0 && (value as usize) < bound {
        Some(value as usize)
    } else {
        None
    }
}

/// One record under construction: its header and the limbs seen so far.
struct Record {
    card: usize,
    kind: usize,
    fields: [u64; FIELDS + 1],
}

struct Row {
    seat: usize,
    tokens: usize,
    options: usize,
}

fn header<S: Source>(flat: &S, base: usize, len: usize, row: usize) -> Result<Row, String> {
    if len < HEADER {
        return Err(format!("row {row} is shorter than its header"));
    }
    let want = |offset: usize, expected: usize, what: &str| {
        if flat.at(base + offset) == expected as f32 {
            Ok(())
        } else {
            Err(format!(
                "row {row} has {what} {} instead of {expected}",
                flat.at(base + offset)
            ))
        }
    };
    want(0, (FAMILY + GENERATION) as usize, "observation family")?;
    want(1, REVISION as usize, "revision")?;
    want(2, SCALARS, "scalar count")?;
    let tokens = index(flat.at(base + 3), usize::MAX)
        .ok_or_else(|| format!("row {row} has a bad token count"))?;
    let options = index(flat.at(base + 4), usize::MAX)
        .ok_or_else(|| format!("row {row} has a bad option count"))?;
    let seat =
        index(flat.at(base + 5), 2).ok_or_else(|| format!("row {row} has a seat outside 0..2"))?;
    let expected = HEADER + SCALARS + TOKEN_FIELDS * tokens + OPTION_FIELDS * options;
    if expected != len {
        return Err(format!(
            "row {row} has {len} floats but its header needs {expected}"
        ));
    }
    Ok(Row {
        seat,
        tokens,
        options,
    })
}

/// The single pass over a batch: `cap` is `None` when only measuring.
struct Walker<O> {
    at: Regions,
    cap: Option<Counts>,
    out: O,
    used: Counts,
    /// Depths of the current row's records, by ordinal.
    depths: Vec<u8>,
}

impl<O: Sink> Walker<O> {
    fn walk<S: Source>(&mut self, flat: S, sizes: &[usize]) -> Result<Counts, String> {
        if let Some(cap) = self.cap
            && sizes.len() > cap.rows
        {
            return Err(format!(
                "{} rows exceed the row capacity {}",
                sizes.len(),
                cap.rows
            ));
        }
        if sizes.iter().sum::<usize>() != flat.len() {
            return Err(format!(
                "row sizes cover {} floats but the buffer holds {}",
                sizes.iter().sum::<usize>(),
                flat.len()
            ));
        }
        let mut base = 0;
        for (row, &len) in sizes.iter().enumerate() {
            self.row(&flat, base, len, row)?;
            base += len;
        }
        Ok(self.used)
    }

    fn row<S: Source>(
        &mut self,
        flat: &S,
        base: usize,
        len: usize,
        row: usize,
    ) -> Result<(), String> {
        let parsed = header(flat, base, len, row)?;
        let rowf = row as f32;
        for i in 0..SCALARS {
            self.out.set(row * SCALARS + i, flat.at(base + HEADER + i));
        }
        self.out.set(self.at.seats + row, parsed.seat as f32);

        self.depths.clear();
        let mut open: Option<Record> = None;
        let tokens = base + HEADER + SCALARS;
        for t in 0..parsed.tokens {
            let [card, group, weight, code, value] =
                flat.chunk::<TOKEN_FIELDS>(tokens + TOKEN_FIELDS * t);
            let card =
                index(card, CARDS).ok_or_else(|| format!("row {row} token {t} has card {card}"))?;
            let group = index(group, GROUPS)
                .ok_or_else(|| format!("row {row} token {t} has group {group}"))?;
            let code = index(code, 1 << 24)
                .ok_or_else(|| format!("row {row} token {t} has code {code}"))?
                as u32;
            if !weight.is_finite() || !value.is_finite() {
                return Err(format!("row {row} token {t} is not finite"));
            }
            if group as u32 != CONTEXT {
                if code >= POOLED_CODES {
                    return Err(format!(
                        "row {row} token {t} has pooled code {code} (limit {POOLED_CODES})"
                    ));
                }
                self.token(rowf, card, group, weight, code, value)?;
            } else if code >= context::FIELD {
                let Some(record) = open.as_mut() else {
                    return Err(format!(
                        "row {row} token {t} is a context field before any record header"
                    ));
                };
                let field = (code - context::FIELD) as usize / 4;
                let limb = (code - context::FIELD) % 4;
                if field == 0 || field > FIELDS {
                    return Err(format!("row {row} token {t} has context field {field}"));
                }
                let Some(limb_value) = index(value, LIMB as usize) else {
                    return Err(format!(
                        "row {row} token {t} has limb value {value}, not a 16-bit integer"
                    ));
                };
                record.fields[field] =
                    record.fields[field].saturating_add((limb_value as u64) << (16 * limb));
            } else if code >= context::HEADER {
                if let Some(record) = open.take() {
                    self.record(row, record)?;
                }
                let kind = (code - context::HEADER) as usize;
                if kind == 0 || kind > KINDS {
                    return Err(format!("row {row} token {t} has record kind {kind}"));
                }
                open = Some(Record {
                    card,
                    kind,
                    fields: [0; FIELDS + 1],
                });
            } else {
                return Err(format!(
                    "row {row} token {t} has context code {code}, neither header nor field"
                ));
            }
        }
        if let Some(record) = open.take() {
            self.record(row, record)?;
        }

        let options = tokens + TOKEN_FIELDS * parsed.tokens;
        for o in 0..parsed.options {
            if let Some(cap) = self.cap
                && self.used.options >= cap.options
            {
                return Err(format!("options exceed the capacity {}", cap.options));
            }
            let k = self.at.options + self.used.options * OPTION_WIDTH;
            self.used.options += 1;
            self.out.set(k, rowf);
            self.out.set(k + 1, 1.0);
            let option = flat.chunk::<OPTION_FIELDS>(options + OPTION_FIELDS * o);
            for (f, &value) in option.iter().enumerate() {
                if index(value, 1 << 24).is_none() {
                    return Err(format!(
                        "row {row} option {o} field {f} is {value}, not a nonnegative integer"
                    ));
                }
                self.out.set(k + 2 + f, value);
            }
        }
        self.used.rows += 1;
        Ok(())
    }

    fn token(
        &mut self,
        rowf: f32,
        card: usize,
        group: usize,
        weight: f32,
        code: u32,
        value: f32,
    ) -> Result<(), String> {
        let stride = match self.cap {
            Some(cap) if self.used.tokens >= cap.tokens => {
                return Err(format!("tokens exceed the capacity {}", cap.tokens));
            }
            Some(cap) => cap.tokens,
            None => 0,
        };
        let n = self.at.tokens + self.used.tokens;
        self.used.tokens += 1;
        self.out.set(n, rowf);
        self.out.set(n + stride, card as f32);
        self.out.set(n + 2 * stride, group as f32);
        self.out.set(n + 3 * stride, weight);
        self.out.set(n + 4 * stride, code as f32);
        self.out.set(n + 5 * stride, value);
        Ok(())
    }

    fn record(&mut self, row: usize, record: Record) -> Result<(), String> {
        let ordinal = self.depths.len() + 1;
        let parent = record.fields[context::Field::Parent as usize];
        if parent >= ordinal as u64 {
            return Err(format!(
                "row {row} record {ordinal} names parent {parent}, itself or a later record"
            ));
        }
        let depth = match parent {
            0 => 0,
            p => (self.depths[p as usize - 1] + 1).min(MAX_DEPTH),
        };
        self.depths.push(depth);
        if let Some(cap) = self.cap
            && self.used.records >= cap.records
        {
            return Err(format!("records exceed the capacity {}", cap.records));
        }
        let r = self.at.records + self.used.records * RECORD_WIDTH;
        self.used.records += 1;
        self.out.set(r, row as f32);
        self.out.set(r + 1, record.card as f32);
        self.out.set(r + 2, record.kind as f32);
        self.out.set(r + 3, f32::from(depth));
        self.out.set(r + 4, 1.0);
        self.out.set(r + 5, 0.0);
        for (field, &value) in record.fields.iter().enumerate().skip(1) {
            self.out.set(
                r + RECORD_FIELD_BASE + field,
                value.min(MAX_FIELD_VALUE) as f32,
            );
        }
        Ok(())
    }

    fn zero_padding(&mut self, cap: Counts) {
        let (at, used) = (&self.at, self.used);
        for i in used.rows * SCALARS..cap.rows * SCALARS {
            self.out.set(i, 0.0);
        }
        for i in at.seats + used.rows..at.tokens {
            self.out.set(i, 0.0);
        }
        for plane in 0..TOKEN_PLANES {
            let base = at.tokens + plane * cap.tokens;
            for i in base + used.tokens..base + cap.tokens {
                self.out.set(i, 0.0);
            }
        }
        for i in at.records + used.records * RECORD_WIDTH..at.options {
            self.out.set(i, 0.0);
        }
        let end = at.options + cap.options * OPTION_WIDTH;
        for i in at.options + used.options * OPTION_WIDTH..end {
            self.out.set(i, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::{Observation, Token};
    use crate::{Match, Player, Prompt, Rng, mode_expansions};

    fn observe_random_root(mode: u8, seed: u64, steps: usize) -> Option<Observation> {
        let mut session = Match::new(mode_expansions(mode).unwrap(), seed).unwrap();
        let mut policy = Rng::new(seed);
        for _ in 0..steps {
            let step = session.random_step(policy.next_u64())?;
            session.apply(step).unwrap();
        }
        let player = match session.prompt() {
            Prompt::Decision(decision) => decision.player,
            Prompt::Turn { player, .. } => *player,
            Prompt::Over(_) => return None,
        };
        Some(session.game().observe(player).unwrap())
    }

    /// Rows from several modes and depths, with the observations they encode.
    fn sample_rows() -> (Vec<Observation>, Vec<f32>, Vec<usize>) {
        let mut observations = Vec::new();
        for (mode, seed, steps) in [
            (0u8, 1u64, 0usize),
            (0, 2, 12),
            (1, 3, 25),
            (2, 4, 40),
            (3, 5, 70),
            (4, 6, 55),
            (7, 7, 90),
            (8, 8, 60),
            (15, 9, 80),
            (16, 10, 45),
            (31, 11, 100),
        ] {
            let mut steps = steps;
            loop {
                if let Some(observation) = observe_random_root(mode, seed, steps) {
                    observations.push(observation);
                    break;
                }
                steps /= 2;
            }
        }
        let rows: Vec<Vec<f32>> = observations.iter().map(Observation::row).collect();
        let sizes = rows.iter().map(Vec::len).collect();
        (observations, rows.concat(), sizes)
    }

    fn packed(flat: &[f32], sizes: &[usize], cap: Counts) -> (Vec<f32>, Counts) {
        let mut out = vec![f32::NAN; cap.width()];
        let used = pack(flat, sizes, cap, &mut out[..]).unwrap();
        (out, used)
    }

    fn roomy(counts: Counts) -> Counts {
        Counts {
            rows: counts.rows + 2,
            tokens: counts.tokens + 17,
            records: counts.records + 5,
            options: counts.options + 9,
        }
    }

    #[test]
    fn real_rows_pack_every_pooled_token_record_and_option_once() {
        let (observations, flat, sizes) = sample_rows();
        let counts = measure(&flat[..], &sizes).unwrap();
        assert_eq!(counts.rows, observations.len());
        let pooled: usize = observations
            .iter()
            .map(|o| {
                o.tokens
                    .iter()
                    .filter(|t| t.group != Group::Context)
                    .count()
            })
            .sum();
        let headers: usize = observations
            .iter()
            .flat_map(|o| &o.tokens)
            .filter(|t| {
                t.group == Group::Context && (context::HEADER..context::FIELD).contains(&t.code)
            })
            .count();
        let options: usize = observations.iter().map(|o| o.options.len()).sum();
        assert_eq!(counts.tokens, pooled);
        assert_eq!(counts.records, headers);
        assert_eq!(counts.options, options);
        assert!(counts.tokens > 100 && counts.records > 0 && counts.options > 0);

        let cap = roomy(counts);
        let (out, used) = packed(&flat, &sizes, cap);
        assert_eq!(used, counts);
        let at = Regions::of(cap);

        let (mut n, mut r, mut k) = (0, 0, 0);
        for (row, observation) in observations.iter().enumerate() {
            assert_eq!(
                &out[row * SCALARS..(row + 1) * SCALARS],
                &observation.scalars[..]
            );
            assert_eq!(out[at.seats + row], observation.viewer.index() as f32);
            let mut ordinal = 0;
            for token in &observation.tokens {
                if token.group == Group::Context {
                    if token.code < context::FIELD {
                        let record = &out[at.records + r * RECORD_WIDTH..][..RECORD_WIDTH];
                        assert_eq!(record[0], row as f32);
                        assert_eq!(record[1], f32::from(token.card));
                        assert_eq!(record[2], (token.code - context::HEADER) as f32);
                        assert_eq!(record[4], 1.0);
                        assert_eq!(record[5], 0.0);
                        let parent = record[RECORD_FIELD_BASE + 1] as usize;
                        assert!(parent <= ordinal, "parent {parent} of record {ordinal}");
                        if parent == 0 {
                            assert_eq!(record[3], 0.0);
                        } else {
                            let above =
                                &out[at.records + (r + parent - ordinal - 1) * RECORD_WIDTH..];
                            assert_eq!(record[3], (above[3] + 1.0).min(f32::from(MAX_DEPTH)));
                        }
                        r += 1;
                        ordinal += 1;
                    }
                    continue;
                }
                let plane = |p: usize| out[at.tokens + p * cap.tokens + n];
                assert_eq!(plane(0), row as f32);
                assert_eq!(plane(1), f32::from(token.card));
                assert_eq!(plane(2), token.group as u8 as f32);
                assert_eq!(plane(3), token.weight);
                assert_eq!(plane(4), token.code as f32);
                assert_eq!(plane(5), token.value);
                n += 1;
            }
            for option in &observation.options {
                let packed = &out[at.options + k * OPTION_WIDTH..][..OPTION_WIDTH];
                assert_eq!(packed[0], row as f32);
                assert_eq!(packed[1], 1.0);
                assert_eq!(&packed[2..], &option[..]);
                k += 1;
            }
        }
        assert_eq!((n, r, k), (counts.tokens, counts.records, counts.options));

        // Padding is zero everywhere.
        assert!(
            out[counts.rows * SCALARS..cap.rows * SCALARS]
                .iter()
                .all(|&v| v == 0.0)
        );
        assert!(
            out[at.seats + counts.rows..at.tokens]
                .iter()
                .all(|&v| v == 0.0)
        );
        for plane in 0..TOKEN_PLANES {
            let base = at.tokens + plane * cap.tokens;
            assert!(
                out[base + counts.tokens..base + cap.tokens]
                    .iter()
                    .all(|&v| v == 0.0)
            );
        }
        assert!(
            out[at.records + counts.records * RECORD_WIDTH..at.options]
                .iter()
                .all(|&v| v == 0.0)
        );
        assert!(
            out[at.options + counts.options * OPTION_WIDTH..]
                .iter()
                .all(|&v| v == 0.0)
        );
        assert!(out.iter().all(|v| !v.is_nan()));
    }

    fn context_token(card: u16, code: u32, value: f32) -> Token {
        Token {
            card,
            group: Group::Context,
            weight: 1.0,
            code,
            value,
        }
    }

    fn field(field: context::Field, limb: u32, value: u32) -> Token {
        context_token(0, context::FIELD + 4 * field as u32 + limb, value as f32)
    }

    fn header_token(kind: context::RecordKind, card: u16) -> Token {
        context_token(card, context::HEADER + kind as u32, 0.0)
    }

    fn synthetic(tokens: Vec<Token>, options: usize) -> Observation {
        Observation {
            viewer: Player::Two,
            sets: 1,
            target: 6,
            scalars: (0..SCALARS).map(|i| i as f32 / 7.0).collect(),
            tokens,
            options: (0..options)
                .map(|o| std::array::from_fn(|f| (o * 100 + f) as f32))
                .collect(),
        }
    }

    #[test]
    fn hand_built_records_recombine_limbs_and_follow_parents() {
        use context::{Field, RecordKind};
        let tokens = vec![
            Token {
                card: 5,
                group: Group::MyHand,
                weight: 0.5,
                code: 9,
                value: 0.25,
            },
            header_token(RecordKind::Execution, 12),
            field(Field::Parent, 0, 0),
            field(Field::Role, 0, 3),
            // 0x1_0002: two limbs.
            field(Field::Scalar, 0, 2),
            field(Field::Scalar, 1, 1),
            header_token(RecordKind::Effect, 0),
            field(Field::Parent, 0, 1),
            // Exceeds the clamp: 0xFFFF + 0xFFFF << 16 + 1 << 32.
            field(Field::Value, 0, 0xFFFF),
            field(Field::Value, 1, 0xFFFF),
            field(Field::Value, 2, 1),
            header_token(RecordKind::Slot, 7),
            field(Field::Parent, 0, 2),
            field(Field::PublicOutcome, 0, 44),
            header_token(RecordKind::Pending, 0),
            field(Field::Parent, 0, 1),
            Token {
                card: 700,
                group: Group::Options,
                weight: 1.0,
                code: 4095,
                value: 0.0,
            },
            header_token(RecordKind::Opaque, 0),
        ];
        let observation = synthetic(tokens, 2);
        let row = observation.row();
        let sizes = [row.len()];
        let counts = measure(&row[..], &sizes).unwrap();
        assert_eq!(
            counts,
            Counts {
                rows: 1,
                tokens: 2,
                records: 5,
                options: 2
            }
        );
        let cap = Counts {
            rows: 1,
            tokens: 2,
            records: 5,
            options: 2,
        };
        let (out, used) = packed(&row, &sizes, cap);
        assert_eq!(used, counts);
        let at = Regions::of(cap);
        let record = |i: usize| &out[at.records + i * RECORD_WIDTH..][..RECORD_WIDTH];
        let f = |i: usize, field: Field| record(i)[RECORD_FIELD_BASE + field as usize];

        assert_eq!(&record(0)[..6], &[0.0, 12.0, 3.0, 0.0, 1.0, 0.0]);
        assert_eq!(f(0, Field::Role), 3.0);
        assert_eq!(f(0, Field::Scalar), 65538.0);
        assert_eq!(&record(1)[..6], &[0.0, 0.0, 4.0, 1.0, 1.0, 0.0]);
        assert_eq!(f(1, Field::Parent), 1.0);
        assert_eq!(f(1, Field::Value), MAX_FIELD_VALUE as f32);
        assert_eq!(&record(2)[..6], &[0.0, 7.0, 6.0, 2.0, 1.0, 0.0]);
        assert_eq!(f(2, Field::PublicOutcome), 44.0);
        assert_eq!(&record(3)[..6], &[0.0, 0.0, 7.0, 1.0, 1.0, 0.0]);
        assert_eq!(&record(4)[..6], &[0.0, 0.0, 19.0, 0.0, 1.0, 0.0]);
        assert!(record(4)[6..].iter().all(|&v| v == 0.0));
        // Unset fields are zero.
        assert_eq!(f(0, Field::Value), 0.0);
        assert_eq!(f(2, Field::Role), 0.0);

        let plane = |p: usize, n: usize| out[at.tokens + p * cap.tokens + n];
        assert_eq!(
            (0..6).map(|p| plane(p, 0)).collect::<Vec<_>>(),
            [0.0, 5.0, Group::MyHand as u8 as f32, 0.5, 9.0, 0.25]
        );
        assert_eq!(
            (0..6).map(|p| plane(p, 1)).collect::<Vec<_>>(),
            [0.0, 700.0, Group::Options as u8 as f32, 1.0, 4095.0, 0.0]
        );
        let option = |k: usize| &out[at.options + k * OPTION_WIDTH..][..OPTION_WIDTH];
        assert_eq!(&option(1)[..2], &[0.0, 1.0]);
        assert_eq!(option(1)[2..], observation.options[1]);
        assert_eq!(out[at.seats], 1.0);
    }

    #[test]
    fn depth_clamps_and_options_land_on_their_rows() {
        use context::{Field, RecordKind};
        let mut tokens = Vec::new();
        for ordinal in 0..10u32 {
            tokens.push(header_token(RecordKind::Item, 0));
            tokens.push(field(Field::Parent, 0, ordinal));
        }
        let first = synthetic(tokens, 0);
        let second = synthetic(vec![header_token(RecordKind::Batch, 3)], 3);
        let third = synthetic(Vec::new(), 1);
        let rows: Vec<Vec<f32>> = [&first, &second, &third].iter().map(|o| o.row()).collect();
        let sizes: Vec<usize> = rows.iter().map(Vec::len).collect();
        let flat = rows.concat();
        let cap = Counts {
            rows: 3,
            tokens: 1,
            records: 11,
            options: 4,
        };
        let (out, used) = packed(&flat, &sizes, cap);
        assert_eq!(
            used,
            Counts {
                rows: 3,
                tokens: 0,
                records: 11,
                options: 4
            }
        );
        let at = Regions::of(cap);
        let depths: Vec<f32> = (0..11)
            .map(|i| out[at.records + i * RECORD_WIDTH + 3])
            .collect();
        assert_eq!(
            depths,
            [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0, 7.0, 0.0]
        );
        assert_eq!(out[at.records + 10 * RECORD_WIDTH], 1.0);
        let option_rows: Vec<f32> = (0..4).map(|k| out[at.options + k * OPTION_WIDTH]).collect();
        assert_eq!(option_rows, [1.0, 1.0, 1.0, 2.0]);
        assert!(out[at.tokens..at.records].iter().all(|&v| v == 0.0));
    }

    fn expect_error(flat: &[f32], sizes: &[usize], cap: Counts, needle: &str) {
        let mut out = vec![0.0; cap.width()];
        let error = pack(flat, sizes, cap, &mut out[..]).unwrap_err();
        assert!(error.contains(needle), "{error:?} lacks {needle:?}");
        if !needle.contains("capacity") && !needle.contains("packed buffer") {
            let error = measure(flat, sizes).unwrap_err();
            assert!(
                error.contains(needle),
                "measure: {error:?} lacks {needle:?}"
            );
        }
    }

    #[test]
    fn caps_and_malformed_rows_are_rejected() {
        use context::{Field, RecordKind};
        let good = synthetic(
            vec![
                Token {
                    card: 1,
                    group: Group::Decks,
                    weight: 1.0,
                    code: 3,
                    value: 0.0,
                },
                header_token(RecordKind::Action, 2),
                field(Field::Parent, 0, 0),
            ],
            1,
        );
        let row = good.row();
        let sizes = [row.len()];
        let fit = Counts {
            rows: 1,
            tokens: 1,
            records: 1,
            options: 1,
        };
        packed(&row, &sizes, fit);
        let mut out = vec![0.0; fit.width() + 1];
        assert!(pack(&row[..], &sizes, fit, &mut out[..]).is_err());
        for (cap, needle) in [
            (Counts { rows: 0, ..fit }, "row capacity"),
            (Counts { tokens: 0, ..fit }, "tokens exceed the capacity"),
            (Counts { records: 0, ..fit }, "records exceed the capacity"),
            (Counts { options: 0, ..fit }, "options exceed the capacity"),
        ] {
            expect_error(&row, &sizes, cap, needle);
        }

        let mutate = |index: usize, value: f32| {
            let mut bad = row.clone();
            bad[index] = value;
            bad
        };
        let token = |t: usize, f: usize| HEADER + SCALARS + TOKEN_FIELDS * t + f;
        let cases: Vec<(Vec<f32>, &str)> = vec![
            (mutate(0, 100.0), "observation family"),
            (mutate(1, 6.0), "revision"),
            (mutate(2, 344.0), "scalar count"),
            (mutate(3, 4.0), "floats but its header needs"),
            (mutate(4, 0.5), "bad option count"),
            (mutate(5, 2.0), "seat"),
            (mutate(token(0, 0), 706.0), "has card"),
            (mutate(token(0, 1), 22.0), "has group"),
            (mutate(token(0, 2), f32::NAN), "not finite"),
            (mutate(token(0, 3), 4096.0), "pooled code"),
            (mutate(token(0, 3), 1.5), "has code"),
            (mutate(token(1, 3), 300.0), "record kind"),
            (mutate(token(1, 3), 100.0), "neither header nor field"),
            (mutate(token(2, 4), 1.0), "itself or a later record"),
            (mutate(token(2, 4), 7.0), "itself or a later record"),
            (mutate(token(2, 4), 65536.0), "not a 16-bit integer"),
            (
                mutate(token(2, 3), context::FIELD as f32),
                "context field 0",
            ),
            (
                mutate(token(2, 3), (context::FIELD + 4 * 51) as f32),
                "context field 51",
            ),
            (
                mutate(token(1, 3), (context::FIELD + 4) as f32),
                "before any record header",
            ),
            (mutate(row.len() - 1, -1.0), "not a nonnegative integer"),
        ];
        for (bad, needle) in cases {
            expect_error(&bad, &[bad.len()], fit, needle);
        }
        expect_error(&row[..row.len() - 1], &sizes, fit, "row sizes cover");
        expect_error(&row[..4], &[4], fit, "shorter than its header");
    }
}
