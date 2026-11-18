//! Source: splitmix64
//! The random numbers P9's generator drew its parts from, in the forms it drew them: a number in `[0, 1)`, one in a
//! range, and a whole number in a range.

/// `SplitMix64`: one 64-bit state, advanced by a constant and mixed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SplitMix(pub u64);

impl SplitMix {
    pub(crate) const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `[0, 1)`, from the top 53 bits.
    #[expect(clippy::cast_precision_loss, reason = "53 bits, which an f64 holds exactly")]
    pub(crate) const fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    pub(crate) fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// A whole number from `lo` to `hi`, both included.
    pub(crate) fn int(&mut self, lo: u32, hi: u32) -> u32 {
        let span = u64::from(hi.saturating_sub(lo)).saturating_add(1);
        let drawn = u32::try_from(self.next_u64().checked_rem(span).unwrap_or(0)).unwrap_or(0);
        lo.saturating_add(drawn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reproduces_the_published_sequence_and_draws_within_its_ranges() {
        let mut generator = SplitMix(1_234_567);
        let want = [6_457_827_717_110_365_317_u64, 3_203_168_211_198_807_973, 9_817_491_932_198_370_423];
        assert_eq!(want.map(|_| generator.next_u64()), want);
        let mut drawn = SplitMix(42);
        for _ in 0..1000 {
            let (unit, ranged, whole) = (drawn.next_f64(), drawn.range(2.0, 3.0), drawn.int(4, 6));
            assert!((0.0..1.0).contains(&unit) && (2.0..3.0).contains(&ranged) && (4..=6).contains(&whole));
        }
        assert_eq!(SplitMix(7).int(9, 9), 9);
    }
}
