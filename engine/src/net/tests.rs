use super::*;
use crate::{Expansions, Match, Prompt, Rng};

fn observe_random_root(mode: u8, seed: u64, steps: usize) -> Option<crate::Observation> {
    let mut session = Match::new(Expansions::from_bits(mode).unwrap(), seed).unwrap();
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

#[test]
fn half_floats_widen_exactly() {
    for (bits, value) in [
        (0x3c00u16, 1.0f32),
        (0xc000, -2.0),
        (0x3555, 0.333_251_95),
        (0x0001, 5.960_464_5e-8),
        (0x8000, -0.0),
        (0x7bff, 65504.0),
        (0xfc00, f32::NEG_INFINITY),
    ] {
        assert_eq!(f16_to_f32(bits).to_bits(), value.to_bits(), "{bits:#06x}");
    }
    assert!(f16_to_f32(0x7e00).is_nan());
}

#[test]
fn parser_rejects_damaged_files() {
    let bytes = synthetic_bytes(Dimensions::TINY, 1, 0.1);
    assert!(Net::from_bytes(&bytes).is_ok());
    assert!(Net::from_bytes(b"INNONET9").unwrap_err().contains("magic"));
    assert!(
        Net::from_bytes(&bytes[..bytes.len() / 2])
            .unwrap_err()
            .contains("bad INNONET file")
    );
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(Net::from_bytes(&extra).unwrap_err().contains("trailing"));
    // INNONET2 with an unknown dtype on the first tensor.
    let mut two = bytes.clone();
    two[..8].copy_from_slice(b"INNONET2");
    let name_len = u32::from_le_bytes(two[12..16].try_into().unwrap()) as usize;
    two.splice(16 + name_len..16 + name_len, 7u32.to_le_bytes());
    assert!(Net::from_bytes(&two).unwrap_err().contains("dtype"));
}

/// Rewrites one metadata scalar of an INNONET1 file in place.
fn tamper(bytes: &mut [u8], name: &str, value: f32) {
    let needle = name.as_bytes();
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("tensor name");
    let data_at = at + needle.len() + 4; // ndim 0, then the scalar
    bytes[data_at..data_at + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn loader_rejects_other_contracts_and_variants() {
    let bytes = synthetic_bytes(Dimensions::TINY, 2, 0.1);
    for (name, what) in [
        ("obs_generation", "generation"),
        ("obs_revision", "revision"),
        ("architecture_id", "architecture"),
        ("artifacts_variant", "variant"),
        ("control_variant", "variant"),
        ("n_groups", "group"),
        ("code_bits", "bit"),
    ] {
        let mut tampered = bytes.clone();
        tamper(&mut tampered, name, 99.0);
        let error = Net::from_bytes(&tampered).unwrap_err();
        assert!(error.contains(what), "{name}: {error}");
    }
}

#[test]
fn zero_network_scores_every_option_from_the_pointer_bias() {
    // All weights zero except the pointer head's state bias and output:
    // Q = sum tanh(b) for every option, whatever the position.
    let mut bytes = synthetic_bytes(Dimensions::TINY, 3, 0.0);
    let mut net = Net::from_bytes(&bytes).unwrap();
    net.pointer_state.b = vec![0.5, -0.25, 1.0, 2.0];
    net.pointer_out.w = vec![1.0, 2.0, -1.0, 0.5];
    let expected: f32 = [0.5f32, -0.25, 1.0, 2.0]
        .iter()
        .zip([1.0, 2.0, -1.0, 0.5])
        .map(|(b, w)| b.tanh() * w)
        .sum();
    let observation = observe_random_root(0, 5, 3).unwrap();
    let q = net.q(&observation).unwrap();
    assert_eq!(q.len(), observation.options.len());
    assert!(q.len() > 1);
    for v in &q {
        assert!((v - expected).abs() < 1e-6, "{v} vs {expected}");
    }
    assert_eq!(net.greedy(&observation), Some(0));
    // The random network still answers with one finite Q per option.
    bytes = synthetic_bytes(Dimensions::TINY, 4, 0.2);
    let net = Net::from_bytes(&bytes).unwrap();
    assert_eq!(net.contract(), (FAMILY + GENERATION, REVISION));
    for (mode, seed, steps) in [(0u8, 6u64, 0usize), (7, 8, 30), (31, 9, 45)] {
        let Some(observation) = observe_random_root(mode, seed, steps) else {
            continue;
        };
        let q = net.q(&observation).unwrap();
        assert_eq!(q.len(), observation.options.len());
        assert!(q.iter().all(|v| v.is_finite()));
        let best = net.greedy(&observation).unwrap();
        assert!(q.iter().all(|&v| v <= q[best]));
        assert_eq!(argmax(&q), Some(best));
    }
}

const TINY_HEADNORM: Dimensions = Dimensions {
    headnorm: true,
    ..Dimensions::TINY
};

#[test]
fn headnorm_network_is_value_plus_centered_advantage() {
    // Zero weights: every norm outputs zero, so A = 0 and Q is the value
    // bias for every option.
    let mut net = Net::from_bytes(&synthetic_bytes(TINY_HEADNORM, 3, 0.0)).unwrap();
    assert!(format!("{net:?}").contains("headnorm: true"));
    let observation = observe_random_root(0, 5, 3).unwrap();
    net.headnorm.as_mut().unwrap().value_out.b = vec![0.75];
    let q = net.q(&observation).unwrap();
    assert!(q.len() > 1 && q.iter().all(|&v| v == 0.75), "{q:?}");
    // Scaling the pointer scales Q's spread about its mean, V, and
    // leaves the mean alone.
    let mut net = Net::from_bytes(&synthetic_bytes(TINY_HEADNORM, 4, 0.5)).unwrap();
    let mean = |q: &[f32]| q.iter().sum::<f32>() / q.len() as f32;
    let q = net.q(&observation).unwrap();
    assert!(q.iter().any(|&v| (v - mean(&q)).abs() > 1e-3), "{q:?}");
    for w in &mut net.pointer_out.w {
        *w *= 3.0;
    }
    let tripled = net.q(&observation).unwrap();
    assert!((mean(&q) - mean(&tripled)).abs() < 1e-5);
    for (a, b) in q.iter().zip(&tripled) {
        assert!((3.0 * (a - mean(&q)) - (b - mean(&tripled))).abs() < 1e-5);
    }
    assert_eq!(net.greedy(&observation), argmax(&tripled));
}

const TINY_HLGAUSS: Dimensions = Dimensions {
    headnorm: true,
    bins: 5,
    ..Dimensions::TINY
};

#[test]
fn hlgauss_network_reads_q_as_the_bin_expectation() {
    // Zero weights: every logit is zero, so Q is the centres' mean, 0.
    let mut net = Net::from_bytes(&synthetic_bytes(TINY_HLGAUSS, 3, 0.0)).unwrap();
    assert!(format!("{net:?}").contains("value_bins: 5"));
    let centres = net.headnorm.as_ref().unwrap().centres.clone();
    assert_eq!(centres.len(), 5);
    let observation = observe_random_root(0, 5, 3).unwrap();
    let q = net.q(&observation).unwrap();
    assert!(q.len() > 1 && q.iter().all(|v| v.abs() < 1e-6), "{q:?}");
    // The value logits alone set every option's distribution: equal mass
    // on bins 3 and 4 gives their mean.
    net.headnorm.as_mut().unwrap().value_out.b = vec![-40.0, -40.0, -40.0, 5.0, 5.0];
    let q = net.q(&observation).unwrap();
    let expected = (centres[3] + centres[4]) / 2.0;
    assert!(q.iter().all(|v| (v - expected).abs() < 1e-5), "{q:?}");
    // A random network: finite Q inside the support, greedy on Q.
    let net = Net::from_bytes(&synthetic_bytes(TINY_HLGAUSS, 4, 0.5)).unwrap();
    for (mode, seed, steps) in [(0u8, 6u64, 0usize), (7, 8, 30), (31, 9, 45)] {
        let Some(observation) = observe_random_root(mode, seed, steps) else {
            continue;
        };
        let q = net.q(&observation).unwrap();
        assert_eq!(q.len(), observation.options.len());
        assert!(
            q.iter().all(|&v| (centres[0]..=centres[4]).contains(&v)),
            "{q:?}"
        );
        assert_eq!(net.greedy(&observation), argmax(&q));
    }
}

const TINY_POLICY: Dimensions = Dimensions {
    policy: true,
    ..TINY_HLGAUSS
};

#[test]
fn policy_network_adds_a_logit_per_option_and_keeps_the_hl_q() {
    // The policy tensor is drawn last, so the HL-Gauss network of the same
    // seed has every other weight.
    let hl = Net::from_bytes(&synthetic_bytes(TINY_HLGAUSS, 4, 0.5)).unwrap();
    let mut net = Net::from_bytes(&synthetic_bytes(TINY_POLICY, 4, 0.5)).unwrap();
    assert!(net.has_policy() && !hl.has_policy());
    assert!(format!("{net:?}").contains("policy: true"));
    let observation = observe_random_root(0, 5, 3).unwrap();
    let (q, policy) = net.evaluate(&observation).unwrap();
    assert_eq!(q, hl.q(&observation).unwrap());
    assert_eq!(hl.evaluate(&observation).unwrap(), (q.clone(), None));
    let policy = policy.unwrap();
    assert_eq!(policy.len(), q.len());
    assert!(policy.iter().all(|v| v.is_finite()));
    assert!(
        policy.iter().any(|&v| (v - policy[0]).abs() > 1e-6),
        "{policy:?}"
    );
    // The policy is linear in its weights.
    for w in net.policy_out.as_mut().unwrap() {
        *w *= 2.0;
    }
    let doubled = net.evaluate(&observation).unwrap().1.unwrap();
    for (a, b) in policy.iter().zip(&doubled) {
        assert!((2.0 * a - b).abs() < 1e-5);
    }
}

#[test]
fn softmax_sampling_runs_from_the_argmax_to_uniform() {
    let logits = [0.0, 2.0, f32::NEG_INFINITY, 1.0];
    let mut rng = Rng::new(9);
    assert_eq!(sample_softmax(&logits, 0.0, &mut rng), Some(1));
    assert_eq!(sample_softmax(&[], 1.0, &mut rng), None);
    let draws = |temperature: f64, rng: &mut Rng| {
        let mut counts = [0usize; 4];
        for _ in 0..20_000 {
            counts[sample_softmax(&logits, temperature, rng).unwrap()] += 1;
        }
        counts
    };
    let cold = draws(0.05, &mut rng);
    assert!(cold[1] > 19_900 && cold[2] == 0, "{cold:?}");
    // softmax([0, 2, -inf, 1]) = [0.090, 0.665, 0, 0.245]
    let unit = draws(1.0, &mut rng);
    assert!((unit[1] as f64 / 20_000.0 - 0.665).abs() < 0.02, "{unit:?}");
    assert!((unit[3] as f64 / 20_000.0 - 0.245).abs() < 0.02, "{unit:?}");
    assert_eq!(unit[2], 0);
    let hot = draws(f64::INFINITY, &mut rng);
    assert!(
        hot.iter()
            .all(|&c| (c as f64 / 20_000.0 - 0.25).abs() < 0.02),
        "{hot:?}"
    );
}

#[test]
fn inverse_temperature_sampling_runs_from_argmax_through_uniform_to_argmin() {
    let logits = [0.0, 2.0, f32::NEG_INFINITY, 1.0];
    let mut rng = Rng::new(11);
    assert_eq!(
        sample_inverse_temperature(&logits, f64::INFINITY, &mut rng),
        Some(1)
    );
    assert_eq!(
        sample_inverse_temperature(&logits, f64::NEG_INFINITY, &mut rng),
        Some(0)
    );
    assert_eq!(sample_inverse_temperature(&[], 1.0, &mut rng), None);
    let draws = |beta: f64, rng: &mut Rng| {
        let mut counts = [0usize; 4];
        for _ in 0..20_000 {
            counts[sample_inverse_temperature(&logits, beta, rng).unwrap()] += 1;
        }
        counts.map(|c| c as f64 / 20_000.0)
    };
    // softmax([0, 2, 1]) = [0.090, 0.665, 0.245] and its mirror for beta -1.
    for (beta, want) in [
        (1.0, [0.090, 0.665, 0.0, 0.245]),
        (0.0, [1.0 / 3.0, 1.0 / 3.0, 0.0, 1.0 / 3.0]),
        (-1.0, [0.665, 0.090, 0.0, 0.245]),
    ] {
        let got = draws(beta, &mut rng);
        assert!(
            got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.02),
            "{beta}: {got:?}"
        );
    }
}

#[test]
fn policy_sampling_plays_the_policy_head_and_only_with_one() {
    let hl = Net::from_bytes(&synthetic_bytes(TINY_HLGAUSS, 4, 0.5)).unwrap();
    let net = Net::from_bytes(&synthetic_bytes(TINY_POLICY, 4, 0.5)).unwrap();
    let observation = observe_random_root(0, 5, 3).unwrap();
    let mut rng = Rng::new(1);
    assert_eq!(hl.policy(&observation).unwrap(), None);
    assert_eq!(hl.sample_policy(&observation, 0.0, &mut rng), None);
    let logits = net.policy(&observation).unwrap().unwrap();
    assert_eq!(
        net.sample_policy(&observation, 0.0, &mut rng),
        argmax(&logits)
    );
    for _ in 0..50 {
        let pick = net.sample_policy(&observation, 2.0, &mut rng).unwrap();
        assert!(pick < logits.len());
    }
}

#[test]
fn hlgauss_loader_requires_exact_ascending_centres() {
    let bytes = synthetic_bytes(TINY_HLGAUSS, 4, 0.5);
    let needle = b"value_centres";
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap();
    // ndim 1, dim 5, then the centres: swap the first two.
    let data_at = at + needle.len() + 8;
    let mut swapped = bytes.clone();
    swapped.copy_within(data_at..data_at + 4, data_at + 4);
    swapped[data_at..data_at + 4].copy_from_slice(&bytes[data_at + 4..data_at + 8]);
    assert!(Net::from_bytes(&swapped).unwrap_err().contains("ascend"));
    let mut scalar = bytes.clone();
    tamper(
        &mut scalar,
        "architecture_id",
        HEADNORM_ARCHITECTURE_ID as f32,
    );
    let error = Net::from_bytes(&scalar).unwrap_err();
    assert!(error.contains("pointer_out has shape"), "{error}");
}

#[test]
fn loader_refuses_a_file_labelled_with_the_other_architecture() {
    let mut headnorm = synthetic_bytes(TINY_HEADNORM, 4, 0.5);
    tamper(&mut headnorm, "architecture_id", ARCHITECTURE_ID as f32);
    let error = Net::from_bytes(&headnorm).unwrap_err();
    assert!(error.contains("unused tensor"), "{error}");
    let mut first = synthetic_bytes(Dimensions::TINY, 4, 0.5);
    tamper(
        &mut first,
        "architecture_id",
        HEADNORM_ARCHITECTURE_ID as f32,
    );
    let error = Net::from_bytes(&first).unwrap_err();
    assert!(error.contains("missing tensor residual_norm0_g"), "{error}");
}

#[test]
fn integer_tensors_decode_with_one_scale_per_row() {
    let mut bytes = b"INNONET2".to_vec();
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(b"w");
    for word in [2u32, 2, 2, 3] {
        bytes.extend(word.to_le_bytes());
    }
    // f16 0.5 and 2.0.
    bytes.extend([0x00, 0x38, 0x00, 0x40]);
    bytes.extend([1i8, -2, 127, 0, -127, 3].map(i8::cast_unsigned));
    let tensors = parse(&bytes).unwrap();
    assert_eq!(tensors[0].1.shape, [2, 3]);
    assert_eq!(tensors[0].1.data, [0.5, -1.0, 63.5, 0.0, -254.0, 6.0]);
    assert!(!tensors[0].1.exact);
    let error = parse(&bytes[..bytes.len() - 1]).unwrap_err();
    assert!(error.contains("data"), "{error}");
}

/// INNONET2 bytes of `tensors`, every tensor of two or more dimensions as
/// 8-bit integers with a power-of-two scale per row, the rest f32.
fn integer_bytes(tensors: &[(String, Tensor)]) -> Vec<u8> {
    let mut out = b"INNONET2".to_vec();
    out.extend((tensors.len() as u32).to_le_bytes());
    for (name, tensor) in tensors {
        let integer = tensor.shape.len() >= 2;
        out.extend((name.len() as u32).to_le_bytes());
        out.extend(name.as_bytes());
        out.extend(u32::from(integer).wrapping_mul(2).to_le_bytes());
        out.extend((tensor.shape.len() as u32).to_le_bytes());
        for &d in &tensor.shape {
            out.extend((d as u32).to_le_bytes());
        }
        if !integer {
            tensor.data.iter().for_each(|v| out.extend(v.to_le_bytes()));
            continue;
        }
        let rows: Vec<&[f32]> = tensor
            .data
            .chunks_exact(tensor.shape[tensor.shape.len() - 1])
            .collect();
        let exponents: Vec<i32> = rows
            .iter()
            .map(|row| {
                let top = row.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                ((top / 127.0).log2().ceil() as i32).clamp(-14, 15)
            })
            .collect();
        for e in &exponents {
            out.extend((((e + 15) as u16) << 10).to_le_bytes());
        }
        for (row, e) in rows.iter().zip(&exponents) {
            let scale = 2f32.powi(*e);
            out.extend(
                row.iter()
                    .map(|v| ((v / scale).round().clamp(-127.0, 127.0) as i8).cast_unsigned()),
            );
        }
    }
    out
}

#[test]
fn compact_files_rebuild_the_card_tables_and_read_integer_weights() {
    let full = parse(&synthetic_bytes(TINY_POLICY, 4, 0.5)).unwrap();
    let (card_dim, token_dim) = (Dimensions::TINY.card_dim, Dimensions::TINY.token_dim);
    let cards = &full
        .iter()
        .find(|(n, _)| n == "card_embeddings")
        .unwrap()
        .1
        .data;
    let mut rng = Rng::new(9);
    let projection: Vec<f32> = (0..GROUPS * card_dim * token_dim)
        .map(|_| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32 - 0.5)
        .collect();
    let mut tables = vec![0.0f32; GROUPS * CARDS * token_dim];
    for (i, value) in tables.iter_mut().enumerate() {
        let (g, c, d) = (
            i / (CARDS * token_dim),
            i / token_dim % CARDS,
            i % token_dim,
        );
        *value = (0..card_dim)
            .map(|k| {
                f64::from(cards[c * card_dim + k])
                    * f64::from(projection[(g * card_dim + k) * token_dim + d])
            })
            .sum::<f64>() as f32;
    }
    // The file with `group_tables` swapped for `name`.
    let with = |name: &str, shape: [usize; 3], data: &[f32]| -> Vec<(String, Tensor)> {
        full.iter()
            .map(|(n, t)| {
                let (n, shape, data) = if n == "group_tables" {
                    (name.to_string(), shape.to_vec(), data.to_vec())
                } else {
                    (n.clone(), t.shape.clone(), t.data.clone())
                };
                (
                    n,
                    Tensor {
                        shape,
                        data,
                        exact: t.exact,
                    },
                )
            })
            .collect()
    };
    let f32_bytes = |tensors: &[(String, Tensor)]| {
        let mut out = b"INNONET1".to_vec();
        out.extend((tensors.len() as u32).to_le_bytes());
        for (name, tensor) in tensors {
            write_tensor(&mut out, name, &tensor.shape, &tensor.data);
        }
        out
    };
    let stored = with("group_tables", [GROUPS, CARDS, token_dim], &tables);
    let factored = with("group_card", [GROUPS, card_dim, token_dim], &projection);
    let stored = Net::from_bytes(&f32_bytes(&stored)).unwrap();
    let rebuilt = Net::from_bytes(&f32_bytes(&factored)).unwrap();
    let tables_agree = rebuilt.group_tables.iter().zip(&stored.group_tables);
    assert!(tables_agree.into_iter().all(|(x, y)| (x - y).abs() < 1e-5));
    // Integer weights round the network without changing what it plays much.
    let integer = Net::from_bytes(&integer_bytes(&factored)).unwrap();
    for (mode, seed, steps) in [(0u8, 6u64, 0usize), (7, 8, 30), (31, 9, 45)] {
        let Some(observation) = observe_random_root(mode, seed, steps) else {
            continue;
        };
        let (exact, rounded) = (
            rebuilt.q(&observation).unwrap(),
            integer.q(&observation).unwrap(),
        );
        let gap = exact
            .iter()
            .zip(&rounded)
            .map(|(u, v)| (u - v).abs())
            .fold(0.0, f32::max);
        assert!(gap > 0.0 && gap < 0.05, "{exact:?} {rounded:?}");
    }
}

#[test]
fn argmax_takes_the_first_of_ties() {
    assert_eq!(argmax(&[]), None);
    assert_eq!(argmax(&[1.0, 3.0, 3.0, 2.0]), Some(1));
    assert_eq!(argmax(&[f32::NEG_INFINITY, f32::NEG_INFINITY]), Some(0));
}
