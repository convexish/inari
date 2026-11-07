//! Exact all-different support and uniform matching samples for <=15 cards.
use crate::{Error, Rng};
use std::collections::HashMap;

fn contradiction() -> Error {
    Error::InvalidSetup("contradictory tracking subsets")
}

fn augment(slot: usize, masks: &[u16], owners: &mut [Option<usize>], seen: &mut u16) -> bool {
    let mut choices = masks[slot] & !*seen;
    while choices != 0 {
        let bit = choices & choices.wrapping_neg();
        choices &= choices - 1;
        if *seen & bit != 0 {
            continue;
        }
        *seen |= bit;
        let card = bit.trailing_zeros() as usize;
        if owners[card].is_none_or(|other| augment(other, masks, owners, seen)) {
            owners[card] = Some(slot);
            return true;
        }
    }
    false
}

fn feasible(masks: &[u16]) -> bool {
    let mut owners = vec![None; masks.len()];
    (0..masks.len()).all(|slot| augment(slot, masks, &mut owners, &mut 0))
}

pub(super) fn close(masks: &mut [u16]) -> Result<(), Error> {
    let n = masks.len();
    if n > 15 || masks.iter().any(|&m| m == 0 || m >> n != 0) {
        return Err(contradiction());
    }
    if n == 0 {
        return Ok(());
    }
    // Most groups consist of singleton facts and one unconstrained remainder.
    loop {
        let singles = masks
            .iter()
            .filter(|m| m.count_ones() == 1)
            .fold(0, |a, m| a | m);
        let mut changed = false;
        for m in masks.iter_mut().filter(|m| m.count_ones() > 1) {
            let pruned = *m & !singles;
            changed |= pruned != *m;
            *m = pruned;
            if pruned == 0 {
                return Err(contradiction());
            }
        }
        if !changed {
            break;
        }
    }
    let singles = masks
        .iter()
        .filter(|m| m.count_ones() == 1)
        .fold(0, |a, m| a | m);
    let remaining = ((1_u16 << n) - 1) & !singles;
    if singles.count_ones() as usize == masks.iter().filter(|m| m.count_ones() == 1).count()
        && masks.iter().all(|&m| m.count_ones() == 1 || m == remaining)
    {
        return Ok(());
    }
    if !feasible(masks) {
        return Err(contradiction());
    }
    let mut probe = masks.to_vec();
    for slot in 0..n {
        if masks[slot].count_ones() <= 1 {
            continue;
        }
        let original = masks[slot];
        let mut choices = original;
        let mut supported = 0;
        while choices != 0 {
            let bit = choices & choices.wrapping_neg();
            choices &= choices - 1;
            probe[slot] = bit;
            if feasible(&probe) {
                supported |= bit;
            }
        }
        masks[slot] = supported;
        probe[slot] = supported;
    }
    Ok(())
}

fn below(rng: &mut Rng, n: u64) -> u64 {
    let threshold = n.wrapping_neg() % n;
    loop {
        let value = rng.next_u64();
        if value >= threshold {
            return value % n;
        }
    }
}

pub(super) fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        let j = below(rng, (i + 1) as u64) as usize;
        items.swap(i, j);
    }
}

pub(super) fn sample(masks: &[u16], rng: &mut Rng) -> Result<Vec<usize>, Error> {
    let n = masks.len();
    if n > 15 || masks.iter().any(|&m| m == 0 || m >> n != 0) {
        return Err(contradiction());
    }
    let singles = masks
        .iter()
        .filter(|m| m.count_ones() == 1)
        .fold(0, |a, m| a | m);
    let remaining = ((1_u16 << n) - 1) & !singles;
    if singles.count_ones() as usize == masks.iter().filter(|m| m.count_ones() == 1).count()
        && masks.iter().all(|&m| m.count_ones() == 1 || m == remaining)
    {
        let mut pool: Vec<_> = (0..n).filter(|i| remaining & (1 << i) != 0).collect();
        shuffle(&mut pool, rng);
        return Ok(masks
            .iter()
            .map(|m| {
                if m.count_ones() == 1 {
                    m.trailing_zeros() as usize
                } else {
                    pool.pop().expect("matching pool")
                }
            })
            .collect());
    }
    let mut order: Vec<_> = (0..n).collect();
    order.sort_by_key(|&i| masks[i].count_ones());
    let full = (1usize << n) - 1;
    let mut counts = vec![0_u64; full + 1];
    counts[full] = 1;
    for used in (0..full).rev() {
        let mut choices = masks[order[used.count_ones() as usize]] & !(used as u16);
        while choices != 0 {
            let bit = choices & choices.wrapping_neg();
            choices &= choices - 1;
            counts[used] += counts[used | bit as usize]; // at most 15! < u64::MAX
        }
    }
    if counts[0] == 0 {
        return Err(contradiction());
    }
    let mut result = vec![0; n];
    let mut used = 0;
    for slot in order {
        let mut draw = below(rng, counts[used]);
        let mut choices = masks[slot] & !(used as u16);
        loop {
            let bit = choices & choices.wrapping_neg();
            let next = used | bit as usize;
            if draw < counts[next] {
                result[slot] = bit.trailing_zeros() as usize;
                used = next;
                break;
            }
            draw -= counts[next];
            choices &= choices - 1;
        }
    }
    Ok(result)
}

/// Uniform physical matching, conditional on retained picks still having a
/// legal identity. Each requirement is (candidate rows, allowed identities).
/// Groups are disjoint all-different cliques of requirement indices. A pinned
/// pick is an ordinary requirement whose allowed identity mask is a singleton.
/// We count physical assignments once, not once per possible pick assignment.
pub(super) fn sample_constrained(
    masks: &[u16],
    requirements: &[(u16, u16)],
    groups: &[Vec<usize>],
    rng: &mut Rng,
) -> Result<Vec<usize>, Error> {
    if requirements.is_empty() && groups.is_empty() {
        // Preserve matching order and RNG consumption for seeded replays.
        return sample(masks, rng);
    }
    let mut counter = Conditional::new(masks, requirements, groups)?;
    let mut domains = vec![0; requirements.len()];
    let mut used = 0;
    if counter.count(used, &domains) == 0 {
        return Err(contradiction());
    }
    let mut result = vec![0; masks.len()];
    for depth in 0..masks.len() {
        let mut draw = below(rng, counter.count(used, &domains));
        let row = counter.order[depth];
        let mut choices = masks[row] & !used;
        while choices != 0 {
            let bit = choices & choices.wrapping_neg();
            choices &= choices - 1;
            let next_domains = counter.admit(row, bit, &domains);
            let count = counter.count(used | bit, &next_domains);
            if draw < count {
                result[row] = bit.trailing_zeros() as usize;
                used |= bit;
                domains = next_domains;
                break;
            }
            draw -= count;
        }
    }
    Ok(result)
}

struct Conditional<'a> {
    masks: &'a [u16],
    requirements: &'a [(u16, u16)],
    groups: &'a [Vec<usize>],
    grouped: Vec<bool>,
    order: Vec<usize>,
    /// Candidate identities that unassigned rows could still supply, before
    /// removing identities already used by another physical row.
    remaining: Vec<Vec<u16>>,
    counts: HashMap<(u16, Vec<u16>), u64>,
    full: u16,
}

impl<'a> Conditional<'a> {
    fn new(
        masks: &'a [u16],
        requirements: &'a [(u16, u16)],
        groups: &'a [Vec<usize>],
    ) -> Result<Self, Error> {
        let n = masks.len();
        if n > 15 || masks.iter().any(|&m| m == 0 || m >> n != 0) {
            return Err(contradiction());
        }
        let full = (1_u16 << n) - 1;
        if requirements
            .iter()
            .any(|&(rows, allowed)| rows == 0 || allowed == 0 || (rows | allowed) & !full != 0)
        {
            return Err(contradiction());
        }
        let mut grouped = vec![false; requirements.len()];
        for group in groups {
            if group.len() > n {
                return Err(contradiction());
            }
            for &pick in group {
                let Some(seen) = grouped.get_mut(pick) else {
                    return Err(contradiction());
                };
                if *seen {
                    return Err(contradiction());
                }
                *seen = true;
            }
        }
        let mut order: Vec<_> = (0..n).collect();
        order.sort_by_key(|&row| masks[row].count_ones());
        let mut remaining = vec![vec![0; requirements.len()]; n + 1];
        for depth in (0..n).rev() {
            let row = order[depth];
            for (pick, &(rows, allowed)) in requirements.iter().enumerate() {
                remaining[depth][pick] = remaining[depth + 1][pick]
                    | if rows & (1 << row) != 0 {
                        masks[row] & allowed
                    } else {
                        0
                    };
            }
        }
        Ok(Self {
            masks,
            requirements,
            groups,
            grouped,
            order,
            remaining,
            counts: HashMap::new(),
            full,
        })
    }

    fn group_feasible(&self, group: &[usize], domains: &[u16]) -> bool {
        let masks: Vec<_> = group.iter().map(|&pick| domains[pick]).collect();
        let mut owners = vec![None; self.masks.len()];
        (0..masks.len()).all(|pick| augment(pick, &masks, &mut owners, &mut 0))
    }

    fn admit(&self, row: usize, bit: u16, domains: &[u16]) -> Vec<u16> {
        let mut next = domains.to_vec();
        for (pick, &(rows, allowed)) in self.requirements.iter().enumerate() {
            if rows & (1 << row) != 0 {
                next[pick] |= bit & allowed;
            }
            // Once an independent pick has a witness, further identities
            // cannot change whether a physical completion is admitted.
            if !self.grouped[pick] && next[pick] != 0 {
                next[pick] = self.full;
            }
        }
        for group in self.groups {
            // The same monotonicity applies after a whole clique already has
            // a distinct assignment. Before that, retain its complete domains;
            // separate nonempty witnesses do not prove joint feasibility.
            if self.group_feasible(group, &next) {
                for &pick in group {
                    next[pick] = self.full;
                }
            }
        }
        next
    }

    fn count(&mut self, used: u16, domains: &[u16]) -> u64 {
        let key = (used, domains.to_vec());
        if let Some(&count) = self.counts.get(&key) {
            return count;
        }
        let depth = used.count_ones() as usize;
        let possible: Vec<_> = domains
            .iter()
            .zip(&self.remaining[depth])
            .map(|(&seen, &remaining)| seen | (remaining & !used))
            .collect();
        let count = if possible.contains(&0)
            || self
                .groups
                .iter()
                .any(|group| !self.group_feasible(group, &possible))
        {
            0
        } else if used == self.full {
            1
        } else {
            let row = self.order[depth];
            let mut choices = self.masks[row] & !used;
            let mut total = 0;
            while choices != 0 {
                let bit = choices & choices.wrapping_neg();
                choices &= choices - 1;
                let next = self.admit(row, bit, domains);
                total += self.count(used | bit, &next); // at most 15!
            }
            total
        };
        self.counts.insert(key, count);
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conditional_oracle(
        masks: &[u16],
        requirements: &[(u16, u16)],
        groups: &[Vec<usize>],
    ) -> Vec<Vec<usize>> {
        fn distinct(domains: &[u16], used: u16) -> bool {
            let Some((&first, rest)) = domains.split_first() else {
                return true;
            };
            (0..15).any(|identity| {
                let bit = 1 << identity;
                first & bit != 0 && used & bit == 0 && distinct(rest, used | bit)
            })
        }
        fn visit(masks: &[u16], prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
            if prefix.len() == masks.len() {
                out.push(prefix.clone());
                return;
            }
            for identity in 0..masks.len() {
                if !prefix.contains(&identity) && masks[prefix.len()] & (1 << identity) != 0 {
                    prefix.push(identity);
                    visit(masks, prefix, out);
                    prefix.pop();
                }
            }
        }
        let mut permutations = Vec::new();
        visit(masks, &mut Vec::new(), &mut permutations);
        permutations.retain(|permutation| {
            let domains: Vec<u16> = requirements
                .iter()
                .map(|&(rows, allowed)| {
                    permutation.iter().enumerate().fold(0, |mask, (row, &id)| {
                        mask | if rows & (1 << row) != 0 {
                            allowed & (1 << id)
                        } else {
                            0
                        }
                    })
                })
                .collect();
            !domains.contains(&0)
                && groups.iter().all(|group| {
                    distinct(&group.iter().map(|&p| domains[p]).collect::<Vec<_>>(), 0)
                })
        });
        permutations
    }

    #[test]
    fn conditional_sampling_matches_small_permutation_oracle() {
        type Case<'a> = (&'a [(u16, u16)], &'a [Vec<usize>]);
        let cases: &[Case<'_>] = &[
            (&[], &[]),
            (&[(0b001, 0b110)], &[]),
            (&[(0b011, 0b110), (0b110, 0b011)], &[]),
            (&[(0b011, 0b011), (0b110, 0b011)], &[vec![0, 1]]),
            (&[(0b111, 0b010), (0b011, 0b011)], &[vec![0, 1]]),
            (&[(0b111, 0b001), (0b111, 0b001)], &[vec![0, 1]]),
            (
                &[(0b111, 0b011), (0b111, 0b011), (0b111, 0b011)],
                &[vec![0, 1, 2]],
            ),
        ];
        for a in 1..8 {
            for b in 1..8 {
                for c in 1..8 {
                    let masks = [a, b, c];
                    for &(requirements, groups) in cases {
                        let expected = conditional_oracle(&masks, requirements, groups);
                        let mut counter = Conditional::new(&masks, requirements, groups).unwrap();
                        assert_eq!(
                            counter.count(0, &vec![0; requirements.len()]),
                            expected.len() as u64,
                            "{masks:?}, {requirements:?}, {groups:?}"
                        );
                        for seed in 0..4 {
                            let mut rng = Rng::new(seed);
                            let before = rng.clone();
                            let actual = sample_constrained(&masks, requirements, groups, &mut rng);
                            if expected.is_empty() {
                                assert!(actual.is_err());
                                assert_eq!(rng, before, "contradictions consume no random draws");
                            } else {
                                assert!(expected.contains(&actual.unwrap()));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn conditional_sampling_keeps_joint_domains_and_pinned_feasibility() {
        let masks = [0b1111; 4];
        let requirements = [(0b0111, 0b0011), (0b0111, 0b0011), (0b1100, 0b0100)];
        let groups = [vec![0, 1]];
        let expected = conditional_oracle(&masks, &requirements, &groups);
        assert_eq!(expected.len(), 8);
        let mut counter = Conditional::new(&masks, &requirements, &groups).unwrap();
        assert_eq!(counter.count(0, &[0; 3]), 8);
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..128 {
            let actual =
                sample_constrained(&masks, &requirements, &groups, &mut Rng::new(seed)).unwrap();
            assert!(expected.contains(&actual));
            seen.insert(actual);
        }
        assert_eq!(
            seen.len(),
            expected.len(),
            "all feasible physical worlds remain reachable"
        );
        // Both picks can individually use C in the first two rows, but the
        // pinned C consumes that identity: the other pick also needs D there.
        let pinned = [(0b0011, 0b1100), (0b0011, 0b0100)];
        let clique = [vec![0, 1]];
        let expected = conditional_oracle(&masks, &pinned, &clique);
        assert_eq!(expected.len(), 4);
        assert!(!expected.contains(&vec![0, 2, 1, 3]));
        assert_eq!(
            Conditional::new(&masks, &pinned, &clique)
                .unwrap()
                .count(0, &[0; 2]),
            4
        );
        for seed in 0..32 {
            let actual = sample_constrained(&masks, &pinned, &clique, &mut Rng::new(seed)).unwrap();
            assert!(expected.contains(&actual));
        }
        assert!(sample_constrained(&masks, &[(0, 1)], &[], &mut Rng::new(0)).is_err());
        assert!(sample_constrained(&masks, &[(1, 0)], &[], &mut Rng::new(0)).is_err());
        assert!(
            sample_constrained(
                &masks,
                &[(15, 15); 3],
                &[vec![0, 1], vec![1, 2]],
                &mut Rng::new(0),
            )
            .is_err()
        );
    }

    #[test]
    fn unconstrained_matching_preserves_historical_rng() {
        for masks in [&[0b1111; 4][..], &[0b0011, 0b0110, 0b1100, 0b1001]] {
            for seed in 0..32 {
                let mut old = Rng::new(seed);
                let mut new = old.clone();
                assert_eq!(
                    sample(masks, &mut old).unwrap(),
                    sample_constrained(masks, &[], &[], &mut new).unwrap()
                );
                assert_eq!(old, new);
            }
        }
    }

    #[test]
    fn hall_pairs_prune_without_any_initial_singleton() {
        let mut masks = [0b0011, 0b0011, 0b1111, 0b1100];
        close(&mut masks).unwrap();
        assert_eq!(masks, [0b0011, 0b0011, 0b1100, 0b1100]);
    }

    #[test]
    fn exhaustive_three_card_support_matches_permutation_oracle() {
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for a in 1..8 {
            for b in 1..8 {
                for c in 1..8 {
                    let input = [a, b, c];
                    let valid: Vec<_> = permutations
                        .iter()
                        .filter(|p| (0..3).all(|i| input[i] & (1 << p[i]) != 0))
                        .collect();
                    let mut actual = input;
                    if valid.is_empty() {
                        assert!(close(&mut actual).is_err());
                        continue;
                    }
                    close(&mut actual).unwrap();
                    let expected =
                        std::array::from_fn(|i| valid.iter().fold(0, |u, p| u | (1 << p[i])));
                    assert_eq!(actual, expected);
                    for seed in 0..8 {
                        let sample = sample(&input, &mut Rng::new(seed)).unwrap();
                        assert!(valid.iter().any(|p| p.as_slice() == sample));
                    }
                }
            }
        }
    }
}
