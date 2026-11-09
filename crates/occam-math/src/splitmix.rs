//! Source: splitmix64
//! The pseudo-random numbers behind the determinism stream: a fixed sequence, the same everywhere.

/// A `SplitMix64` generator.
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A generator starting from `seed`.
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next number in the sequence.
    pub const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let z = (self.0 ^ (self.0 >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reproduces_the_published_sequence() {
        let mut generator = SplitMix64::new(1_234_567);
        let want = [
            6_457_827_717_110_365_317_u64,
            3_203_168_211_198_807_973,
            9_817_491_932_198_370_423,
            4_593_380_528_125_082_431,
            16_408_922_859_458_223_821,
        ];
        assert_eq!(want.map(|_| generator.next_u64()), want);
    }
}
