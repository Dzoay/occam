//! Source: fnv-1a
//! Hashing what the kernel computes. A NaN's sign and payload are not deterministic, so every NaN is hashed as one
//! pattern (`docs/DETERMINISM.md`).

/// The one pattern every NaN is hashed as. A literal, because `f64::NAN`'s bits are not guaranteed.
pub const CANONICAL_NAN: u64 = 0x7FF8_0000_0000_0000;

/// The multiplier of 64-bit FNV-1a.
const PRIME: u64 = 0x0000_0100_0000_01B3;

/// The bits of `x`, with every NaN replaced by [`CANONICAL_NAN`]. The two zeros stay distinct.
#[must_use]
#[inline]
pub const fn canonical_bits(x: f64) -> u64 {
    if x.is_nan() { CANONICAL_NAN } else { x.to_bits() }
}

/// A 64-bit FNV-1a digest of what is written to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digest(u64);

impl Digest {
    /// The digest of nothing.
    pub const EMPTY: Self = Self(0xCBF2_9CE4_8422_2325);

    /// Writes bytes.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(PRIME);
        }
    }

    /// Writes a word, as its little-endian bytes.
    #[inline]
    pub fn write_u64(&mut self, word: u64) {
        self.write_bytes(&word.to_le_bytes());
    }

    /// Writes a number, as its canonical bits.
    #[inline]
    pub fn write_value(&mut self, x: f64) {
        self.write_u64(canonical_bits(x));
    }

    /// The digest of everything written so far.
    #[must_use]
    pub const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_of(bytes: &[u8]) -> u64 {
        let mut digest = Digest::EMPTY;
        digest.write_bytes(bytes);
        digest.finish()
    }

    #[test]
    fn every_nan_is_hashed_as_one_pattern() {
        let nans = [
            0x7FF8_0000_0000_0000_u64, // the quiet NaN Rust's `f64::NAN` usually is
            0xFFF8_0000_0000_0000,     // the sign set: what 0.0 / 0.0 gives on x86-64
            0x7FF0_0000_0000_0001,     // signalling
            0xFFFF_FFFF_FFFF_FFFF,     // every bit set
            0x7FF8_DEAD_BEEF_0001,     // a payload
        ];
        for bits in nans {
            assert_eq!(canonical_bits(f64::from_bits(bits)), CANONICAL_NAN, "{bits:#x}");
        }
    }

    #[test]
    fn every_other_number_keeps_its_bits() {
        let numbers = [0.0, -0.0, 1.0, -1.5, f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE, f64::from_bits(1)];
        for x in numbers {
            assert_eq!(canonical_bits(x), x.to_bits(), "{x:e}");
        }
    }

    #[test]
    fn the_digest_is_fnv_1a_64() {
        assert_eq!(digest_of(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(digest_of(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(digest_of(b"foobar"), 0x8594_4171_F739_67E8);
    }

    #[test]
    fn a_word_is_written_as_its_little_endian_bytes() {
        let mut digest = Digest::EMPTY;
        digest.write_u64(0x0102_0304_0506_0708);
        assert_eq!(digest.finish(), digest_of(&[8, 7, 6, 5, 4, 3, 2, 1]));
    }

    #[test]
    fn a_number_is_written_as_its_canonical_bits() {
        let mut negative_nan = Digest::EMPTY;
        negative_nan.write_value(f64::from_bits(0xFFF8_0000_0000_0000));
        assert_eq!(negative_nan.finish(), digest_of(&CANONICAL_NAN.to_le_bytes()));
        let mut one = Digest::EMPTY;
        one.write_value(1.0);
        assert_eq!(one.finish(), digest_of(&1.0_f64.to_bits().to_le_bytes()));
    }

    #[test]
    fn the_order_of_what_is_written_matters() {
        assert_ne!(digest_of(&[1, 2]), digest_of(&[2, 1]));
    }
}
