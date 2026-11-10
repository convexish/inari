//! Torch-free inference for the option-pointer Q networks
//! (train/td_net.py `QNetNext1`, `QNetNext1V2` as architecture id 2,
//! `QNetNext1V2HLGauss` as id 3 and `QNetNext1V2HLGaussPolicy` as id 4) from
//! the INNONET1/INNONET2 files that train/export_td_weights.py
//! `export_next_arrays` writes.
//!
//! The forward runs on the same packed buffer as Torch ([`pack::pack`] with
//! exact capacities), so one row's Q vector agrees with `QNetNext1.packed_q`
//! up to float summation order. Weights stay `f32` in memory; an f16 or
//! integer file is widened once at load. Per-group card tables
//! (`groups × cards × token_dim`, stored, or built from the card embeddings
//! and each group's projection in compact files) and the pointer head's card
//! tables are precomputed at load; a pooled
//! token's code projection is summed from its set bits per call, which costs
//! at most twelve `token_dim`-wide adds per token instead of a 44 MB table.
//!
//! One evaluation of the production-size network (width 384, eight residual
//! blocks, token and pointer dims 128) is about 5–10 million multiply-adds,
//! dominated by the trunk: a few milliseconds on one core.
use std::collections::HashMap;
use std::path::Path;

use crate::observation::context::Field;
use crate::observation::pack::{self, CARDS, Counts, OPTION_WIDTH, RECORD_WIDTH, TOKEN_PLANES};
use crate::observation::{
    FAMILY, GENERATION, GROUPS, Group, OPTION_FIELDS, REVISION, SCALARS, field,
};

/// Numeric id of the `obs_next1_option_pointer_v1` architecture in exports.
pub const ARCHITECTURE_ID: u32 = 1;
/// Numeric id of `obs_next1_option_pointer_v2_headnorm`: pre-norm SiLU
/// blocks, a final LayerNorm and `Q = V + A - mean A` with normalized heads.
pub const HEADNORM_ARCHITECTURE_ID: u32 = 2;
/// Numeric id of `obs_next1_option_pointer_v2_hlgauss`: the head-norm network
/// whose value and advantage outputs are logits over value bins, combined as
/// `V + A - mean A` per bin; Q is the softmax's expectation of the bin centres.
pub const HLGAUSS_ARCHITECTURE_ID: u32 = 3;
/// Numeric id of `obs_next1_option_pointer_v2_hlgauss_policy`: the HL-Gauss
/// network plus one policy logit per option, a linear readout of the
/// activation the advantage output reads.
pub const POLICY_ARCHITECTURE_ID: u32 = 4;
/// Torch's `nn.LayerNorm` default.
const LAYER_NORM_EPS: f32 = 1e-5;
/// Rules variants the observation encoder is coupled to, as the export codes them.
pub const ARTIFACTS_VARIANT: u32 = 2;
pub const CONTROL_VARIANT: u32 = 1;
/// Pooled token codes are bitmasks below `1 << CODE_BITS`.
pub const CODE_BITS: usize = 12;
/// Token features: the code bits and the value.
pub const TOKEN_FEATURES: usize = CODE_BITS + 1;
/// Groups whose tokens pool by identity; the last group is the Context stream.
pub const POOLED_GROUPS: usize = Group::Context as usize;
/// Features of one context record (`obs_next.record_features`).
pub const RECORD_FEATURES: usize = 241;
/// Features of one option (`obs_next.option_features`).
pub const OPTION_FEATURES: usize = 232;

const CONTEXT: usize = Group::Context as usize;
const N_RECORD_KINDS: usize = pack::KINDS + 1;
const N_BACKS: usize = 72;
const MAX_DEPTH: usize = pack::MAX_DEPTH as usize;
const RECORD_ROLES: usize = 16;
const RECORD_SCALAR_KINDS: usize = 8;
const RECORD_SCALARS: usize = 16;
const RECORD_VERBS: usize = 16;
const RECORD_ZONES: usize = 24;
const RECORD_EFFECTS: usize = 4;
const RECORD_ACTION_KINDS: usize = 4;
const RECORD_COUNT_CAP: f32 = 20.0;
const PLAYER_FIELDS: [Field; 5] = [
    Field::Executor,
    Field::Controller,
    Field::Demander,
    Field::SourceOwner,
    Field::Owner,
];
const COUNT_FIELDS: [Field; 3] = [Field::Count, Field::Limit, Field::Performed];
const BINARY_FIELDS: [Field; 8] = [
    Field::Known,
    Field::Optional,
    Field::Shared,
    Field::Endorsed,
    Field::Promoted,
    Field::Failed,
    Field::Append,
    Field::Choice,
];
const N_OPTION_KINDS: usize = 31;
const N_ZONES: usize = 21;
const N_ICONS: usize = 10;
const N_VERBS: usize = 14;
const N_SPECIALS: usize = 26;
const N_SLOTS: usize = 10;
const N_OWNERS: usize = 3;
const N_NUMBERS: usize = 16;

/// `y = W x + b` with `W` row-major `out × in` (Torch's `nn.Linear` orientation).
struct Linear {
    w: Vec<f32>,
    b: Vec<f32>,
    in_dim: usize,
}

impl Linear {
    fn apply(&self, x: &[f32]) -> Vec<f32> {
        let mut out = self.b.clone();
        self.add_into(x, &mut out);
        out
    }

    /// `out += W x`.
    fn add_into(&self, x: &[f32], out: &mut [f32]) {
        debug_assert_eq!(x.len(), self.in_dim);
        for (o, row) in out.iter_mut().zip(self.w.chunks_exact(self.in_dim)) {
            *o += dot(row, x);
        }
    }
}

/// Eight accumulator lanes so the compiler may vectorize the reduction.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let (a8, a_tail) = a.as_chunks::<8>();
    let (b8, b_tail) = b.as_chunks::<8>();
    let mut acc = [0.0f32; 8];
    for (ca, cb) in a8.iter().zip(b8) {
        for l in 0..8 {
            acc[l] += ca[l] * cb[l];
        }
    }
    let tail: f32 = a_tail.iter().zip(b_tail).map(|(x, y)| x * y).sum();
    acc.iter().sum::<f32>() + tail
}

/// An affine LayerNorm over the whole vector.
struct Norm {
    g: Vec<f32>,
    b: Vec<f32>,
}

impl Norm {
    fn apply(&self, x: &mut [f32]) {
        let n = x.len() as f32;
        let mean = x.iter().sum::<f32>() / n;
        let variance = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n;
        let scale = 1.0 / (variance + LAYER_NORM_EPS).sqrt();
        for ((v, g), b) in x.iter_mut().zip(&self.g).zip(&self.b) {
            *v = (*v - mean) * scale * g + b;
        }
    }
}

/// The head-norm network's parts beyond the first network's.
struct HeadNorm {
    /// Each residual block's input norm.
    block_norms: Vec<Norm>,
    /// The trunk's final norm.
    trunk_norm: Norm,
    value_hidden: Linear,
    value_norm: Norm,
    /// One output per value bin, like `Net::pointer_out`.
    value_out: Linear,
    pointer_state_norm: Norm,
    pointer_option_norm: Norm,
    /// The value bins' centres, ascending; empty for a scalar head.
    centres: Vec<f32>,
}

fn add_scaled(out: &mut [f32], add: &[f32], scale: f32) {
    for (o, a) in out.iter_mut().zip(add) {
        *o += a * scale;
    }
}

/// One parsed tensor, widened to `f32`; `exact` when the file stored `f32`.
#[derive(Debug)]
struct Tensor {
    shape: Vec<usize>,
    data: Vec<f32>,
    exact: bool,
}

/// IEEE half to single, by hand (no `half` dependency in this crate).
fn f16_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let fraction = u32::from(bits & 0x3ff);
    if exponent == 0 {
        // Zero or subnormal: fraction × 2^-24.
        let magnitude = fraction as f32 / 16_777_216.0;
        return if sign == 0 { magnitude } else { -magnitude };
    }
    if exponent == 31 {
        return f32::from_bits(sign | 0x7f80_0000 | (fraction << 13));
    }
    f32::from_bits(sign | ((exponent + 112) << 23) | (fraction << 13))
}

/// INNONET1 (all f32) or INNONET2 (per-tensor dtype: 0 f32, 1 f16, 2 i8 with
/// one f16 scale per row of the last dimension, the scales first):
/// `magic, u32 count, then per tensor u32 name_len, name, [u32 dtype],
/// u32 ndim, u32 dims..., little-endian data`.
fn parse(bytes: &[u8]) -> Result<Vec<(String, Tensor)>, String> {
    let err = |m: &str| format!("bad INNONET file: {m}");
    let with_dtypes = match bytes.get(..8) {
        Some(b"INNONET1") => false,
        Some(b"INNONET2") => true,
        _ => return Err(err("missing magic")),
    };
    let mut at = 8;
    let u32_at = |at: &mut usize| -> Result<usize, String> {
        let raw = bytes.get(*at..*at + 4).ok_or_else(|| err("truncated"))?;
        *at += 4;
        Ok(u32::from_le_bytes(raw.try_into().expect("four bytes")) as usize)
    };
    let n = u32_at(&mut at)?;
    let mut out = Vec::with_capacity(n.min(1 << 12));
    for _ in 0..n {
        let name_len = u32_at(&mut at)?;
        let name = std::str::from_utf8(bytes.get(at..at + name_len).ok_or_else(|| err("name"))?)
            .map_err(|_| err("name utf8"))?
            .to_string();
        at += name_len;
        let dtype = if with_dtypes { u32_at(&mut at)? } else { 0 };
        let ndim = u32_at(&mut at)?;
        let mut shape = Vec::with_capacity(ndim.min(8));
        for _ in 0..ndim {
            shape.push(u32_at(&mut at)?);
        }
        let count = shape
            .iter()
            .try_fold(1usize, |n, &d| n.checked_mul(d))
            .ok_or_else(|| err("shape overflow"))?
            .max(1);
        let (data, exact) = match dtype {
            0 => {
                let end = at
                    .checked_add(4 * count)
                    .filter(|&end| end <= bytes.len())
                    .ok_or_else(|| err("data"))?;
                let raw = &bytes[at..end];
                at = end;
                let (chunks, _) = raw.as_chunks::<4>();
                (
                    chunks.iter().map(|b| f32::from_le_bytes(*b)).collect(),
                    true,
                )
            }
            1 => {
                let end = at
                    .checked_add(2 * count)
                    .filter(|&end| end <= bytes.len())
                    .ok_or_else(|| err("data"))?;
                let raw = &bytes[at..end];
                at = end;
                let (chunks, _) = raw.as_chunks::<2>();
                (
                    chunks
                        .iter()
                        .map(|b| f16_to_f32(u16::from_le_bytes(*b)))
                        .collect(),
                    false,
                )
            }
            2 => {
                let row = shape.last().copied().unwrap_or(1).max(1);
                let rows = count / row;
                let end = rows
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(count))
                    .and_then(|n| at.checked_add(n))
                    .filter(|&end| end <= bytes.len())
                    .ok_or_else(|| err("data"))?;
                let (scales, values) = bytes[at..end].split_at(2 * rows);
                at = end;
                let (scales, _) = scales.as_chunks::<2>();
                let data = values
                    .chunks_exact(row)
                    .zip(scales)
                    .flat_map(|(values, scale)| {
                        let scale = f16_to_f32(u16::from_le_bytes(*scale));
                        values
                            .iter()
                            .map(move |&q| f32::from(q.cast_signed()) * scale)
                    })
                    .collect();
                (data, false)
            }
            _ => return Err(err("unknown dtype")),
        };
        out.push((name, Tensor { shape, data, exact }));
    }
    if at != bytes.len() {
        return Err(err("trailing bytes"));
    }
    Ok(out)
}

/// Tensors of one file, taken by name as the network is assembled.
struct Tensors(HashMap<String, Tensor>);

impl Tensors {
    fn take(&mut self, name: &str) -> Result<Tensor, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing tensor {name}"))
    }

    /// A metadata scalar, which must be stored exactly.
    fn scalar(&mut self, name: &str) -> Result<f32, String> {
        let tensor = self.take(name)?;
        if !tensor.exact {
            return Err(format!("metadata tensor {name} must be f32"));
        }
        tensor
            .data
            .first()
            .copied()
            .ok_or_else(|| format!("metadata tensor {name} is empty"))
    }

    fn integer(&mut self, name: &str) -> Result<usize, String> {
        let value = self.scalar(name)?;
        if value.fract() != 0.0 || !(0.0..=16_777_216.0).contains(&value) {
            return Err(format!(
                "metadata tensor {name} is {value}, not a small integer"
            ));
        }
        Ok(value as usize)
    }

    fn expect(&mut self, name: &str, expected: usize, what: &str) -> Result<(), String> {
        let value = self.integer(name)?;
        if value != expected {
            return Err(format!("{what} {value} does not match {expected}"));
        }
        Ok(())
    }

    fn shaped(&mut self, name: &str, shape: &[usize]) -> Result<Vec<f32>, String> {
        let tensor = self.take(name)?;
        if tensor.shape != shape {
            return Err(format!(
                "tensor {name} has shape {:?}, expected {shape:?}",
                tensor.shape
            ));
        }
        Ok(tensor.data)
    }

    fn linear(
        &mut self,
        name: &str,
        out_dim: usize,
        in_dim: usize,
        bias: bool,
    ) -> Result<Linear, String> {
        let w = self.shaped(&format!("{name}_W"), &[out_dim, in_dim])?;
        let b = if bias {
            self.shaped(&format!("{name}_b"), &[out_dim])?
        } else {
            vec![0.0; out_dim]
        };
        Ok(Linear { w, b, in_dim })
    }

    fn norm(&mut self, name: &str, dim: usize) -> Result<Norm, String> {
        Ok(Norm {
            g: self.shaped(&format!("{name}_g"), &[dim])?,
            b: self.shaped(&format!("{name}_b"), &[dim])?,
        })
    }
}

/// A loaded network.
///
/// Its `Debug` form is the architecture summary, not the weights.
pub struct Net {
    token_dim: usize,
    pointer_dim: usize,
    width: usize,
    pool_scale: f32,
    iteration: u32,
    /// `groups × cards × token_dim`: each group's embedding of each card.
    group_tables: Vec<f32>,
    /// `pooled groups × TOKEN_FEATURES × token_dim`: code-bit and value weights.
    group_code: Vec<f32>,
    /// `pooled groups × token_dim`.
    group_bias: Vec<f32>,
    context: Linear,
    stem: Linear,
    residual: Vec<(Linear, Linear)>,
    pointer_state: Linear,
    pointer_features: Linear,
    /// `cards × pointer_dim`: the pointer head's embedding of the named card.
    pointer_cards: Vec<f32>,
    /// `cards × pointer_dim`: the same for the second card.
    pointer_seconds: Vec<f32>,
    /// One output (Q, or the advantage) or one logit per value bin, without bias.
    pointer_out: Linear,
    /// `pointer_dim` weights of the policy logit, for the policy architecture.
    policy_out: Option<Vec<f32>>,
    /// `Some` for the head-norm architectures.
    headnorm: Option<HeadNorm>,
}

impl std::fmt::Debug for Net {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Net")
            .field("width", &self.width)
            .field("residual_blocks", &self.residual.len())
            .field("token_dim", &self.token_dim)
            .field("pointer_dim", &self.pointer_dim)
            .field("headnorm", &self.headnorm.is_some())
            .field("value_bins", &self.pointer_out.b.len())
            .field("policy", &self.policy_out.is_some())
            .field("iteration", &self.iteration)
            .finish_non_exhaustive()
    }
}

impl Net {
    pub fn load(path: &Path) -> Result<Net, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Net::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Net, String> {
        let mut t = Tensors(parse(bytes)?.into_iter().collect());
        t.expect(
            "obs_generation",
            (FAMILY + GENERATION) as usize,
            "observation generation",
        )?;
        t.expect("obs_revision", REVISION as usize, "observation revision")?;
        let architecture = t.integer("architecture_id")? as u32;
        if ![
            ARCHITECTURE_ID,
            HEADNORM_ARCHITECTURE_ID,
            HLGAUSS_ARCHITECTURE_ID,
            POLICY_ARCHITECTURE_ID,
        ]
        .contains(&architecture)
        {
            return Err(format!(
                "architecture id {architecture} is not {ARCHITECTURE_ID}, \
                 {HEADNORM_ARCHITECTURE_ID}, {HLGAUSS_ARCHITECTURE_ID} or \
                 {POLICY_ARCHITECTURE_ID}"
            ));
        }
        t.expect(
            "artifacts_variant",
            ARTIFACTS_VARIANT as usize,
            "Artifacts rules variant",
        )?;
        t.expect(
            "control_variant",
            CONTROL_VARIANT as usize,
            "control rules variant",
        )?;
        t.expect("n_groups", GROUPS, "token group count")?;
        t.expect("code_bits", CODE_BITS, "code bit count")?;
        let card_dim = t.integer("card_dim")?;
        let token_dim = t.integer("token_dim")?;
        let pointer_dim = t.integer("pointer_dim")?;
        let width = t.integer("width")?;
        let blocks = t.integer("residual_blocks")?;
        if card_dim == 0 || token_dim == 0 || pointer_dim == 0 || width == 0 {
            return Err("network dimensions must be positive".into());
        }
        let pool_scale = t.scalar("pool_scale")?;
        if !pool_scale.is_finite() || pool_scale <= 0.0 {
            return Err(format!("pool scale {pool_scale} is not positive"));
        }
        let iteration = t.integer("iter").unwrap_or(0) as u32;
        let cards = t.shaped("card_embeddings", &[CARDS, card_dim])?;
        // Compact files store each group's card projection instead of its table.
        let group_tables = if t.0.contains_key("group_card") {
            let projection = t.shaped("group_card", &[GROUPS, card_dim, token_dim])?;
            let mut tables = vec![0.0; GROUPS * CARDS * token_dim];
            for (table, projection) in tables
                .chunks_exact_mut(CARDS * token_dim)
                .zip(projection.chunks_exact(card_dim * token_dim))
            {
                for (row, card) in table
                    .chunks_exact_mut(token_dim)
                    .zip(cards.chunks_exact(card_dim))
                {
                    for (&x, weights) in card.iter().zip(projection.chunks_exact(token_dim)) {
                        for (out, &w) in row.iter_mut().zip(weights) {
                            *out += x * w;
                        }
                    }
                }
            }
            tables
        } else {
            t.shaped("group_tables", &[GROUPS, CARDS, token_dim])?
        };
        let group_code = t.shaped("group_code", &[POOLED_GROUPS, TOKEN_FEATURES, token_dim])?;
        let group_bias = t.shaped("group_bias", &[POOLED_GROUPS, token_dim])?;
        let context = t.linear("context_features", token_dim, RECORD_FEATURES, true)?;
        let stem = t.linear("stem", width, SCALARS + GROUPS * token_dim, false)?;
        let residual = (0..blocks)
            .map(|i| {
                Ok((
                    t.linear(&format!("residual_inner{i}"), width, width, true)?,
                    t.linear(&format!("residual_outer{i}"), width, width, true)?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let pointer_state = t.linear("pointer_state", pointer_dim, width, true)?;
        let pointer_features = t.linear("pointer_features", pointer_dim, OPTION_FEATURES, false)?;
        let pointer_card = t.linear("pointer_card", pointer_dim, card_dim, false)?;
        let pointer_second = t.linear("pointer_second", pointer_dim, card_dim, false)?;
        let centres = if [HLGAUSS_ARCHITECTURE_ID, POLICY_ARCHITECTURE_ID].contains(&architecture) {
            value_centres(t.take("value_centres")?)?
        } else {
            Vec::new()
        };
        let policy_out = if architecture == POLICY_ARCHITECTURE_ID {
            Some(t.shaped("policy_out", &[pointer_dim])?)
        } else {
            None
        };
        let bins = centres.len().max(1);
        // A scalar output is stored as a vector; bin outputs as a matrix.
        let outputs = |t: &mut Tensors, name: &str| -> Result<Vec<f32>, String> {
            if centres.is_empty() {
                t.shaped(name, &[pointer_dim])
            } else {
                t.shaped(name, &[bins, pointer_dim])
            }
        };
        let pointer_out = Linear {
            w: outputs(&mut t, "pointer_out")?,
            b: vec![0.0; bins],
            in_dim: pointer_dim,
        };
        let headnorm = if architecture != ARCHITECTURE_ID {
            Some(HeadNorm {
                block_norms: (0..blocks)
                    .map(|i| t.norm(&format!("residual_norm{i}"), width))
                    .collect::<Result<_, String>>()?,
                trunk_norm: t.norm("trunk_norm", width)?,
                value_hidden: t.linear("value_hidden", pointer_dim, width, true)?,
                value_norm: t.norm("value_norm", pointer_dim)?,
                value_out: Linear {
                    w: outputs(&mut t, "value_out")?,
                    b: t.shaped("value_out_b", &[bins])?,
                    in_dim: pointer_dim,
                },
                pointer_state_norm: t.norm("pointer_state_norm", pointer_dim)?,
                pointer_option_norm: t.norm("pointer_option_norm", pointer_dim)?,
                centres,
            })
        } else {
            None
        };
        if let Some(name) = t.0.keys().min() {
            return Err(format!("unused tensor {name}"));
        }
        let table = |linear: &Linear| -> Vec<f32> {
            cards
                .chunks_exact(card_dim)
                .flat_map(|card| linear.apply(card))
                .collect()
        };
        let net = Net {
            token_dim,
            pointer_dim,
            width,
            pool_scale,
            iteration,
            group_tables,
            group_code,
            group_bias,
            context,
            stem,
            residual,
            pointer_state,
            pointer_features,
            pointer_cards: table(&pointer_card),
            pointer_seconds: table(&pointer_second),
            pointer_out,
            policy_out,
            headnorm,
        };
        if net.weights().any(|w| !w.is_finite()) {
            return Err("network weights are not finite".into());
        }
        Ok(net)
    }

    fn weights(&self) -> impl Iterator<Item = f32> + '_ {
        let linears = [
            &self.context,
            &self.stem,
            &self.pointer_state,
            &self.pointer_features,
        ]
        .into_iter()
        .chain(self.residual.iter().flat_map(|(a, b)| [a, b]))
        .chain([&self.pointer_out])
        .chain(
            self.headnorm
                .iter()
                .flat_map(|h| [&h.value_hidden, &h.value_out]),
        );
        let norms = self.headnorm.iter().flat_map(|h| {
            h.block_norms.iter().chain([
                &h.trunk_norm,
                &h.value_norm,
                &h.pointer_state_norm,
                &h.pointer_option_norm,
            ])
        });
        let centres = self.headnorm.iter().flat_map(|h| &h.centres);
        linears
            .flat_map(|l| l.w.iter().chain(&l.b))
            .chain(norms.flat_map(|n| n.g.iter().chain(&n.b)))
            .chain(centres)
            .chain(&self.group_tables)
            .chain(&self.group_code)
            .chain(&self.group_bias)
            .chain(&self.pointer_cards)
            .chain(&self.pointer_seconds)
            .chain(self.policy_out.iter().flatten())
            .copied()
    }

    /// The observation `(generation, revision)` this network consumes.
    pub fn contract(&self) -> (u32, u32) {
        (FAMILY + GENERATION, REVISION)
    }

    /// The trainer iteration the export recorded.
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// `(width, residual blocks, token_dim, pointer_dim)`.
    pub fn dimensions(&self) -> (usize, usize, usize, usize) {
        (
            self.width,
            self.residual.len(),
            self.token_dim,
            self.pointer_dim,
        )
    }

    /// One Q per option, in the observation's option order.
    pub fn q(&self, observation: &crate::Observation) -> Result<Vec<f32>, String> {
        self.q_row(&observation.row())
    }

    /// The option with the highest Q (the first on ties), if there is one.
    pub fn greedy(&self, observation: &crate::Observation) -> Option<usize> {
        argmax(&self.q(observation).ok()?)
    }

    /// One Q per option of a flat observation row (`Observation::row`).
    pub fn q_row(&self, row: &[f32]) -> Result<Vec<f32>, String> {
        Ok(self.evaluate_row(row)?.0)
    }

    /// Whether the network has a policy output ([`POLICY_ARCHITECTURE_ID`]).
    pub fn has_policy(&self) -> bool {
        self.policy_out.is_some()
    }

    /// One policy logit per option, for a network with a policy output
    /// ([`Net::has_policy`]); None otherwise.
    pub fn policy(&self, observation: &crate::Observation) -> Result<Option<Vec<f32>>, String> {
        Ok(self.evaluate(observation)?.1)
    }

    /// An option drawn from `softmax(policy / temperature)` with `rng`
    /// ([`sample_softmax`]): the policy output's own move choice, the play
    /// the policy head is trained for (search never reads it). Temperature
    /// 0 plays the best option, infinity uniformly at random. None without
    /// a policy output, on a refused evaluation, or with no option.
    pub fn sample_policy(
        &self,
        observation: &crate::Observation,
        temperature: f64,
        rng: &mut crate::Rng,
    ) -> Option<usize> {
        sample_softmax(&self.policy(observation).ok()??, temperature, rng)
    }

    /// One Q and, with a policy output, one policy logit per option.
    pub fn evaluate(
        &self,
        observation: &crate::Observation,
    ) -> Result<(Vec<f32>, Option<Vec<f32>>), String> {
        self.evaluate_row(&observation.row())
    }

    /// [`Net::evaluate`] on a flat observation row (`Observation::row`).
    pub fn evaluate_row(&self, row: &[f32]) -> Result<(Vec<f32>, Option<Vec<f32>>), String> {
        let sizes = [row.len()];
        let counts: Counts = pack::measure(row, &sizes)?;
        let mut packed = vec![0.0f32; counts.width()];
        pack::pack(row, &sizes, counts, &mut packed[..])?;
        Ok(self.forward_packed(&packed, counts))
    }

    /// The forward on one packed row (`pack::pack` with exact capacities):
    /// Q per option, and the policy logits if the network has them.
    fn forward_packed(&self, packed: &[f32], counts: Counts) -> (Vec<f32>, Option<Vec<f32>>) {
        let d = self.token_dim;
        let scalars = &packed[..SCALARS];
        let tokens_at = SCALARS + 1;
        let records_at = tokens_at + TOKEN_PLANES * counts.tokens;
        let options_at = records_at + RECORD_WIDTH * counts.records;
        let plane =
            |p: usize| &packed[tokens_at + p * counts.tokens..tokens_at + (p + 1) * counts.tokens];
        let (cards, groups, weights, codes, values) =
            (plane(1), plane(2), plane(3), plane(4), plane(5));

        let mut summary = vec![0.0f32; GROUPS * d];
        let mut hidden = vec![0.0f32; d];
        for t in 0..counts.tokens {
            let (card, group) = (
                cards[t] as usize,
                (groups[t] as usize).min(POOLED_GROUPS - 1),
            );
            let code = codes[t] as u32;
            hidden.copy_from_slice(&self.group_tables[(group * CARDS + card) * d..][..d]);
            add_scaled(&mut hidden, &self.group_bias[group * d..][..d], 1.0);
            let code_rows = &self.group_code[group * TOKEN_FEATURES * d..][..TOKEN_FEATURES * d];
            for bit in 0..CODE_BITS {
                if code >> bit & 1 == 1 {
                    add_scaled(&mut hidden, &code_rows[bit * d..][..d], 1.0);
                }
            }
            add_scaled(&mut hidden, &code_rows[CODE_BITS * d..][..d], values[t]);
            relu(&mut hidden);
            add_scaled(
                &mut summary[group * d..][..d],
                &hidden,
                weights[t] * self.pool_scale,
            );
        }
        let mut features = vec![0.0f32; RECORD_FEATURES];
        for record in packed[records_at..options_at].as_chunks::<RECORD_WIDTH>().0 {
            let (card, present) = (record[1] as usize, record[4]);
            record_features(record, &mut features);
            hidden.copy_from_slice(&self.group_tables[(CONTEXT * CARDS + card) * d..][..d]);
            add_scaled(&mut hidden, &self.context.b, 1.0);
            self.context.add_into(&features, &mut hidden);
            relu(&mut hidden);
            add_scaled(
                &mut summary[CONTEXT * d..][..d],
                &hidden,
                present * self.pool_scale,
            );
        }

        let mut input = Vec::with_capacity(SCALARS + GROUPS * d);
        input.extend_from_slice(scalars);
        input.extend_from_slice(&summary);
        let mut x = self.stem.apply(&input);
        for (i, (inner, outer)) in self.residual.iter().enumerate() {
            let branch = match &self.headnorm {
                Some(h) => {
                    let mut normed = x.clone();
                    h.block_norms[i].apply(&mut normed);
                    let mut branch = inner.apply(&normed);
                    silu(&mut branch);
                    branch
                }
                None => {
                    let mut branch = inner.apply(&x);
                    relu(&mut branch);
                    branch
                }
            };
            outer.add_into(&branch, &mut x);
            add_scaled(&mut x, &outer.b, 1.0);
        }
        if let Some(h) = &self.headnorm {
            h.trunk_norm.apply(&mut x);
        }
        let mut state = self.pointer_state.apply(&x);
        if let Some(h) = &self.headnorm {
            h.pointer_state_norm.apply(&mut state);
        }

        let p = self.pointer_dim;
        let bins = self.pointer_out.b.len();
        let mut option_features = vec![0.0f32; OPTION_FEATURES];
        // Each option's outputs: Q for the first network, else its advantage
        // (one per bin).
        let mut out = Vec::with_capacity(counts.options * bins);
        let mut policy = self
            .policy_out
            .as_ref()
            .map(|_| Vec::with_capacity(counts.options));
        for option in packed[options_at..].as_chunks::<OPTION_WIDTH>().0 {
            let fields = &option[2..2 + OPTION_FIELDS];
            let (card, second) = (fields[field::CARD] as usize, fields[field::SECOND] as usize);
            self::option_features(fields, &mut option_features);
            // The first network sums the state term first, as it always did.
            let mut h = match &self.headnorm {
                Some(_) => vec![0.0f32; p],
                None => state.clone(),
            };
            self.pointer_features.add_into(&option_features, &mut h);
            add_scaled(&mut h, &self.pointer_cards[card * p..][..p], 1.0);
            add_scaled(&mut h, &self.pointer_seconds[second * p..][..p], 1.0);
            match &self.headnorm {
                Some(norms) => {
                    norms.pointer_option_norm.apply(&mut h);
                    add_scaled(&mut h, &state, 1.0);
                    silu(&mut h);
                }
                None => {
                    for v in &mut h {
                        *v = v.tanh();
                    }
                }
            }
            let at = out.len();
            out.resize(at + bins, 0.0);
            self.pointer_out.add_into(&h, &mut out[at..]);
            if let (Some(weights), Some(policy)) = (&self.policy_out, &mut policy) {
                policy.push(weights.iter().zip(&h).map(|(w, v)| w * v).sum());
            }
        }
        let Some(h) = &self.headnorm else {
            return (out, policy);
        };
        if out.is_empty() {
            return (out, policy);
        }
        let mut hidden = h.value_hidden.apply(&x);
        h.value_norm.apply(&mut hidden);
        silu(&mut hidden);
        // Per bin, V minus the options' mean advantage.
        let mut shift = h.value_out.apply(&hidden);
        for (bin, s) in shift.iter_mut().enumerate() {
            let total = out.iter().skip(bin).step_by(bins).sum::<f32>();
            *s -= total / counts.options as f32;
        }
        for logits in out.chunks_exact_mut(bins) {
            add_scaled(logits, &shift, 1.0);
        }
        if h.centres.is_empty() {
            return (out, policy);
        }
        let q = out
            .chunks_exact(bins)
            .map(|logits| expectation(logits, &h.centres))
            .collect();
        (q, policy)
    }
}

/// Ascending, exactly stored value-bin centres.
fn value_centres(tensor: Tensor) -> Result<Vec<f32>, String> {
    if !tensor.exact || tensor.shape.len() != 1 || tensor.data.len() < 2 {
        return Err("value_centres must be at least two f32 values".into());
    }
    if !tensor.data.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("value_centres must ascend".into());
    }
    Ok(tensor.data)
}

/// `softmax(logits) · centres`.
fn expectation(logits: &[f32], centres: &[f32]) -> f32 {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let (mut total, mut weighted) = (0.0f32, 0.0f32);
    for (&logit, &centre) in logits.iter().zip(centres) {
        let e = (logit - max).exp();
        total += e;
        weighted += e * centre;
    }
    weighted / total
}

fn relu(x: &mut [f32]) {
    for v in x {
        *v = v.max(0.0);
    }
}

fn silu(x: &mut [f32]) {
    for v in x {
        *v /= 1.0 + (-*v).exp();
    }
}

/// The first index of the largest value.
pub fn argmax(q: &[f32]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, &v) in q.iter().enumerate() {
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.map(|(i, _)| i)
}

/// An index drawn with probability `softmax(logits / temperature)`:
/// temperature 0 (or below) is [`argmax`], an infinite temperature is
/// uniform, and a non-finite logit is skipped. None for no logits.
pub fn sample_softmax(logits: &[f32], temperature: f64, rng: &mut crate::Rng) -> Option<usize> {
    if logits.is_empty() {
        return None;
    }
    if temperature <= 0.0 {
        return argmax(logits);
    }
    let unit = |rng: &mut crate::Rng| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    if temperature.is_infinite() {
        return Some((unit(rng) * logits.len() as f64) as usize).map(|i| i.min(logits.len() - 1));
    }
    let top = logits
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f64> = logits
        .iter()
        .map(|&v| {
            if v.is_finite() {
                (f64::from(v - top) / temperature).exp()
            } else {
                0.0
            }
        })
        .collect();
    let total: f64 = weights.iter().sum();
    if total.is_nan() || total <= 0.0 || total.is_infinite() {
        return argmax(logits);
    }
    let mut u = unit(rng) * total;
    for (i, w) in weights.iter().enumerate() {
        if u < *w {
            return Some(i);
        }
        u -= w;
    }
    weights.iter().rposition(|w| *w > 0.0)
}

/// An index drawn with probability proportional to `exp(beta * logit)`:
/// `beta` is the inverse temperature, so positive values prefer high logits,
/// 0 is uniform, negative values prefer low logits, and +/-infinity play the
/// first largest or smallest logit. A non-finite logit is skipped. None for
/// no finite logits.
pub fn sample_inverse_temperature(
    logits: &[f32],
    beta: f64,
    rng: &mut crate::Rng,
) -> Option<usize> {
    let finite = || logits.iter().enumerate().filter(|(_, v)| v.is_finite());
    if beta == f64::INFINITY {
        return argmax(logits);
    }
    if beta == f64::NEG_INFINITY {
        return finite()
            .fold(None, |best: Option<(usize, f32)>, (i, &v)| {
                if best.is_none_or(|(_, b)| v < b) {
                    Some((i, v))
                } else {
                    best
                }
            })
            .map(|(i, _)| i);
    }
    let reference = if beta >= 0.0 {
        finite().map(|(_, &v)| v).fold(f32::NEG_INFINITY, f32::max)
    } else {
        finite().map(|(_, &v)| v).fold(f32::INFINITY, f32::min)
    };
    if !reference.is_finite() {
        return None;
    }
    let weights: Vec<f64> = logits
        .iter()
        .map(|&v| {
            if v.is_finite() {
                (beta * f64::from(v - reference)).exp()
            } else {
                0.0
            }
        })
        .collect();
    let total: f64 = weights.iter().sum();
    let mut u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * total;
    for (i, w) in weights.iter().enumerate() {
        if u < *w {
            return Some(i);
        }
        u -= w;
    }
    weights.iter().rposition(|w| *w > 0.0)
}

/// Writes clamped one-hots and scalars into a feature vector in order.
struct Features<'a> {
    out: &'a mut [f32],
    at: usize,
}

impl Features<'_> {
    fn one_hot(&mut self, value: f32, n: usize) {
        let index = (value.max(0.0) as usize).min(n - 1);
        self.out[self.at + index] = 1.0;
        self.at += n;
    }

    fn scalar(&mut self, value: f32) {
        self.out[self.at] = value;
        self.at += 1;
    }
}

/// `obs_next.record_features` of one packed record (row, card, kind, depth,
/// present, 0, fields 1..=50).
fn record_features(record: &[f32], out: &mut [f32]) {
    out.fill(0.0);
    let field = |f: Field| record[pack::RECORD_FIELD_BASE + f as usize];
    let mut w = Features { out, at: 0 };
    w.one_hot(record[2], N_RECORD_KINDS);
    w.one_hot(field(Field::Back), N_BACKS + 1);
    w.one_hot(record[3], MAX_DEPTH + 1);
    w.one_hot(field(Field::Role), RECORD_ROLES);
    for f in PLAYER_FIELDS {
        w.one_hot(field(f), 3);
    }
    w.one_hot(field(Field::ScalarKind), RECORD_SCALAR_KINDS);
    w.one_hot(field(Field::Scalar), RECORD_SCALARS);
    w.scalar(field(Field::Scalar) / 10.0);
    for f in COUNT_FIELDS {
        w.scalar(field(f).clamp(0.0, RECORD_COUNT_CAP) / 10.0);
    }
    w.one_hot(field(Field::Verb), RECORD_VERBS);
    w.one_hot(field(Field::SourceZone), RECORD_ZONES);
    w.one_hot(field(Field::Destination), RECORD_ZONES);
    w.one_hot(field(Field::Effect), RECORD_EFFECTS);
    for f in BINARY_FIELDS {
        w.scalar(f32::from(field(f) != 0.0));
    }
    w.one_hot(field(Field::ActionKind), RECORD_ACTION_KINDS);
    w.scalar(field(Field::Value) / 11.0);
    debug_assert_eq!(w.at, RECORD_FEATURES);
}

/// `obs_next.option_features` of one option's thirteen fields.
fn option_features(fields: &[f32], out: &mut [f32]) {
    out.fill(0.0);
    let mut w = Features { out, at: 0 };
    w.one_hot(fields[field::KIND], N_OPTION_KINDS);
    w.one_hot(fields[field::ZONE], N_ZONES);
    w.one_hot(fields[field::BACK], N_BACKS + 1);
    w.one_hot(fields[field::NUMBER], N_NUMBERS);
    w.scalar(fields[field::NUMBER] / 10.0);
    let colors = fields[field::COLORS] as u32;
    for bit in 0..5 {
        w.scalar((colors >> bit & 1) as f32);
    }
    w.one_hot(fields[field::ICON], N_ICONS);
    w.one_hot(fields[field::VERB], N_VERBS);
    w.one_hot(fields[field::TOKEN], N_SPECIALS);
    w.one_hot(fields[field::SLOT], N_SLOTS);
    w.scalar(fields[field::SLOT] / 10.0);
    w.one_hot(fields[field::OWNER], N_OWNERS);
    w.one_hot(fields[field::SECOND_ZONE], N_ZONES);
    debug_assert_eq!(w.at, OPTION_FEATURES);
}

/// Dimensions of a synthetic network (see [`synthetic_bytes`]).
#[derive(Clone, Copy, Debug)]
pub struct Dimensions {
    pub card_dim: usize,
    pub token_dim: usize,
    pub pointer_dim: usize,
    pub width: usize,
    pub blocks: usize,
    /// The head-norm architecture rather than the first network.
    pub headnorm: bool,
    /// Value bins of the HL-Gauss head-norm architecture; 1 for a scalar head.
    pub bins: usize,
    /// A policy logit per option (needs value bins).
    pub policy: bool,
}

impl Dimensions {
    /// The smallest configuration worth loading.
    pub const TINY: Self = Self {
        card_dim: 4,
        token_dim: 4,
        pointer_dim: 4,
        width: 8,
        blocks: 1,
        headnorm: false,
        bins: 1,
        policy: false,
    };
}

/// Appends one f32 tensor in INNONET1 layout.
fn write_tensor(out: &mut Vec<u8>, name: &str, shape: &[usize], data: &[f32]) {
    debug_assert_eq!(shape.iter().product::<usize>().max(1), data.len());
    out.extend((name.len() as u32).to_le_bytes());
    out.extend(name.as_bytes());
    out.extend((shape.len() as u32).to_le_bytes());
    for &d in shape {
        out.extend((d as u32).to_le_bytes());
    }
    for v in data {
        out.extend(v.to_le_bytes());
    }
}

/// A complete INNONET1 file of a network with small random weights
/// (uniform in `±scale`), for tests and tools that need a loadable network
/// without a trained checkpoint. `scale` 0 gives an all-zero network.
pub fn synthetic_bytes(dims: Dimensions, seed: u64, scale: f32) -> Vec<u8> {
    let Dimensions {
        card_dim,
        token_dim,
        pointer_dim,
        width,
        blocks,
        headnorm,
        bins,
        policy,
    } = dims;
    assert!(
        bins >= 1 && (bins == 1 || headnorm),
        "value bins need the head-norm network"
    );
    assert!(!policy || bins > 1, "a policy needs value bins");
    let architecture = match (headnorm, bins, policy) {
        (false, _, _) => ARCHITECTURE_ID,
        (true, 1, _) => HEADNORM_ARCHITECTURE_ID,
        (true, _, false) => HLGAUSS_ARCHITECTURE_ID,
        (true, _, true) => POLICY_ARCHITECTURE_ID,
    };
    // A scalar output is a vector, bin outputs a matrix.
    let outputs = if bins == 1 {
        vec![pointer_dim]
    } else {
        vec![bins, pointer_dim]
    };
    let mut rng = crate::Rng::new(seed);
    let mut random = |n: usize| -> Vec<f32> {
        (0..n)
            .map(|_| ((rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0) * scale)
            .collect()
    };
    let mut tensors: Vec<(&str, Vec<usize>, Vec<f32>)> = vec![
        ("obs_generation", vec![], vec![(FAMILY + GENERATION) as f32]),
        ("obs_revision", vec![], vec![REVISION as f32]),
        ("architecture_id", vec![], vec![architecture as f32]),
        ("artifacts_variant", vec![], vec![ARTIFACTS_VARIANT as f32]),
        ("control_variant", vec![], vec![CONTROL_VARIANT as f32]),
        ("n_groups", vec![], vec![GROUPS as f32]),
        ("code_bits", vec![], vec![CODE_BITS as f32]),
        ("card_dim", vec![], vec![card_dim as f32]),
        ("token_dim", vec![], vec![token_dim as f32]),
        ("pointer_dim", vec![], vec![pointer_dim as f32]),
        ("width", vec![], vec![width as f32]),
        ("residual_blocks", vec![], vec![blocks as f32]),
        ("pool_scale", vec![], vec![1.0 / 16.0]),
        ("iter", vec![], vec![0.0]),
    ];
    let mut weight =
        |name: &'static str, shape: Vec<usize>, random: &mut dyn FnMut(usize) -> Vec<f32>| {
            let n = shape.iter().product();
            tensors.push((name, shape, random(n)));
        };
    weight("card_embeddings", vec![CARDS, card_dim], &mut random);
    weight("group_tables", vec![GROUPS, CARDS, token_dim], &mut random);
    weight(
        "group_code",
        vec![POOLED_GROUPS, TOKEN_FEATURES, token_dim],
        &mut random,
    );
    weight("group_bias", vec![POOLED_GROUPS, token_dim], &mut random);
    weight(
        "context_features_W",
        vec![token_dim, RECORD_FEATURES],
        &mut random,
    );
    weight("context_features_b", vec![token_dim], &mut random);
    weight(
        "stem_W",
        vec![width, SCALARS + GROUPS * token_dim],
        &mut random,
    );
    weight("pointer_state_W", vec![pointer_dim, width], &mut random);
    weight("pointer_state_b", vec![pointer_dim], &mut random);
    weight(
        "pointer_features_W",
        vec![pointer_dim, OPTION_FEATURES],
        &mut random,
    );
    weight("pointer_card_W", vec![pointer_dim, card_dim], &mut random);
    weight("pointer_second_W", vec![pointer_dim, card_dim], &mut random);
    weight("pointer_out", outputs.clone(), &mut random);
    let block_names: Vec<[String; 4]> = (0..blocks)
        .map(|i| {
            [
                format!("residual_inner{i}_W"),
                format!("residual_inner{i}_b"),
                format!("residual_outer{i}_W"),
                format!("residual_outer{i}_b"),
            ]
        })
        .collect();
    let mut extra: Vec<(String, Vec<usize>)> = Vec::new();
    if headnorm {
        let norms = (0..blocks)
            .map(|i| (format!("residual_norm{i}"), width))
            .chain([
                ("trunk_norm".to_string(), width),
                ("value_norm".to_string(), pointer_dim),
                ("pointer_state_norm".to_string(), pointer_dim),
                ("pointer_option_norm".to_string(), pointer_dim),
            ]);
        for (name, dim) in norms {
            extra.push((format!("{name}_g"), vec![dim]));
            extra.push((format!("{name}_b"), vec![dim]));
        }
        extra.push(("value_hidden_W".into(), vec![pointer_dim, width]));
        extra.push(("value_hidden_b".into(), vec![pointer_dim]));
        extra.push(("value_out".into(), outputs));
        extra.push(("value_out_b".into(), vec![bins]));
    }
    if policy {
        extra.push(("policy_out".into(), vec![pointer_dim]));
    }
    // Centres of equal bins on [-1.1, 1.1], as the trainer's HL-Gauss head.
    let centres: Vec<f32> = (0..bins)
        .map(|i| -1.1 + 2.2 * (i as f32 + 0.5) / bins as f32)
        .collect();
    let mut out = Vec::new();
    out.extend(b"INNONET1");
    let count = tensors.len() + 4 * blocks + extra.len() + usize::from(bins > 1);
    out.extend((count as u32).to_le_bytes());
    for (name, shape, data) in &tensors {
        write_tensor(&mut out, name, shape, data);
    }
    for names in &block_names {
        write_tensor(&mut out, &names[0], &[width, width], &random(width * width));
        write_tensor(&mut out, &names[1], &[width], &random(width));
        write_tensor(&mut out, &names[2], &[width, width], &random(width * width));
        write_tensor(&mut out, &names[3], &[width], &random(width));
    }
    for (name, shape) in &extra {
        write_tensor(&mut out, name, shape, &random(shape.iter().product()));
    }
    if bins > 1 {
        write_tensor(&mut out, "value_centres", &[bins], &centres);
    }
    out
}

#[cfg(test)]
mod tests;
