/// Splitmix64 stream and shuffle algorithm; changing either breaks seeded replays.
/// Simulations clone this state; search must use a separate generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub(crate) fn shuffle<T>(&mut self, cards: &mut [T]) {
        for i in (1..cards.len()).rev() {
            let j = (self.next_u64() % (i + 1) as u64) as usize;
            cards.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_singleton_shuffles_preserve_the_stream() {
        let mut rng = Rng::new(42);
        let before = rng.clone();
        rng.shuffle(&mut [] as &mut [u8]);
        let mut singleton = [7];
        rng.shuffle(&mut singleton);
        assert_eq!(singleton, [7]);
        assert_eq!(rng, before);
    }

    #[test]
    fn matches_production_frozen_vectors() {
        // engine/tests/rng_parity.rs: preserve the original cross-language stream.
        for (seed, expected) in [
            (
                0,
                [
                    16294208416658607535,
                    7960286522194355700,
                    487617019471545679,
                    17909611376780542444,
                ],
            ),
            (
                1,
                [
                    10451216379200822465,
                    13757245211066428519,
                    17911839290282890590,
                    8196980753821780235,
                ],
            ),
            (
                42,
                [
                    13679457532755275413,
                    2949826092126892291,
                    5139283748462763858,
                    6349198060258255764,
                ],
            ),
        ] {
            let mut rng = Rng::new(seed);
            for value in expected {
                assert_eq!(rng.next_u64(), value, "seed {seed}");
            }
        }
    }
}
