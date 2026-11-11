//! Source: payne-hanek-1983
//! Payne and Hanek's reduction of a large argument by π/2: the argument's 53-bit mantissa times a window of 2/π's
//! bits, in integers, so that it is exact however large the argument is.

use super::constants::{PIO2_HI, PIO2_LO, TWO_OVER_PI};
use crate::exact::{fast_two_sum, two_product};

/// The arguments the fast reduction handles. Above this, `k` could reach 2^20 and `k` times a piece of π/2 stop
/// being exact.
pub const LIMIT: f64 = 1_048_576.0;

/// The bits of 2/π multiplied in: enough that the dropped bits change the fraction by under 2^-137.
const WINDOW: u32 = 192;

/// The top bits of the reduced fraction kept: 53 for the high double and 64 for the low.
const KEPT: u32 = 117;

/// A 256-bit unsigned integer, `hi * 2^128 + lo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Wide {
    hi: u128,
    lo: u128,
}

impl Wide {
    /// The number of bits up to and including the highest set one.
    const fn bit_length(self) -> u32 {
        if self.hi == 0 {
            128_u32.wrapping_sub(self.lo.leading_zeros())
        } else {
            256_u32.wrapping_sub(self.hi.leading_zeros())
        }
    }

    /// The lowest 128 bits of `self >> shift`, for a shift below 256.
    fn shr_low(self, shift: u32) -> u128 {
        if shift >= 128 {
            self.hi.checked_shr(shift.wrapping_sub(128)).unwrap_or(0)
        } else if shift == 0 {
            self.lo
        } else {
            // The two parts share no bits, so adding them joins them.
            let low = self.lo.checked_shr(shift).unwrap_or(0);
            low.wrapping_add(self.hi.checked_shl(128_u32.wrapping_sub(shift)).unwrap_or(0))
        }
    }
}

/// 2^`k` as a double, for `k` from -1022 to 1023.
fn power_of_two(k: i32) -> f64 {
    let biased = u64::try_from(k.saturating_add(1023)).unwrap_or(0);
    f64::from_bits(biased << 52)
}

/// Bits `first` to `first + 191` of 2/π, bit 1 being the first after the binary point, as three 64-bit words, the most
/// significant first.
fn window(first: u32) -> [u64; 3] {
    let skip = first.saturating_sub(1);
    let (word, offset) = (usize::try_from(skip / 64).unwrap_or(usize::MAX), skip % 64);
    let at = |index: usize| TWO_OVER_PI.get(index).copied().unwrap_or(0);
    let piece = |index: usize| {
        let high = at(index).checked_shl(offset).unwrap_or(0);
        let low = at(index.saturating_add(1)).checked_shr(64_u32.saturating_sub(offset)).unwrap_or(0);
        // The two parts share no bits, so adding them joins them.
        if offset == 0 { at(index) } else { high.wrapping_add(low) }
    };
    [piece(word), piece(word.saturating_add(1)), piece(word.saturating_add(2))]
}

/// `m * w`, for `m` below 2^53 and `w` of 192 bits, most significant word first.
fn product(m: u64, [w2, w1, w0]: [u64; 3]) -> Wide {
    let m = u128::from(m);
    let (top, middle, bottom) =
        (m.wrapping_mul(u128::from(w2)), m.wrapping_mul(u128::from(w1)), m.wrapping_mul(u128::from(w0)));
    let (lo, carry) = bottom.overflowing_add(middle << 64);
    let hi = top.wrapping_add(middle >> 64).wrapping_add(u128::from(carry));
    Wide { hi, lo }
}

/// The reduced argument of a finite `x` above [`LIMIT`] in size, as a double-double `(r, rr)` with `|r|` at most π/4,
/// and the quadrant `q` as `0.0`, `1.0`, `2.0` or `-1.0`: `x = (4n + q) π/2 + r + rr` for some integer `n`. NaN for
/// an argument that is not finite, and zeros for one that is small enough for the fast reduction.
#[expect(
    clippy::cast_precision_loss,
    reason = "the high part has 53 bits, and the low part rounds to nearest by design"
)]
pub fn reduce(x: f64) -> (f64, f64, f64) {
    if !x.is_finite() {
        return (f64::NAN, f64::NAN, 0.0);
    }
    if x.abs() <= LIMIT {
        return (0.0, 0.0, 0.0);
    }
    let bits = x.abs().to_bits();
    let exponent = i32::try_from((bits >> 52) & 0x7FF).unwrap_or(0).wrapping_sub(1075);
    // The fraction and the implicit leading bit share no bits, so adding them joins them.
    let mantissa = (bits & 0x000F_FFFF_FFFF_FFFF).wrapping_add(0x0010_0000_0000_0000);
    let first = u32::try_from(exponent.saturating_sub(1).max(1)).unwrap_or(1);
    let all = product(mantissa, window(first));
    // The binary point of the product, counted up from its lowest bit: from 190 to 224 here, so above 128.
    let last = i64::from(first).wrapping_add(i64::from(WINDOW)).wrapping_sub(1);
    let point = u32::try_from(last.wrapping_sub(i64::from(exponent))).unwrap_or(0);
    let above = point.wrapping_sub(128);
    let mut quadrant = all.hi.checked_shr(above).unwrap_or(0) & 3;
    let fraction = Wide { hi: all.hi & 1_u128.checked_shl(above).unwrap_or(0).wrapping_sub(1), lo: all.lo };
    let over_half = fraction.hi.checked_shr(above.wrapping_sub(1)).unwrap_or(0) & 1 == 1;
    let (magnitude, mut negative) = if over_half {
        quadrant = quadrant.wrapping_add(1) & 3;
        let borrow = u128::from(fraction.lo != 0);
        let hi = 1_u128.checked_shl(above).unwrap_or(0).wrapping_sub(fraction.hi).wrapping_sub(borrow);
        (Wide { hi, lo: fraction.lo.wrapping_neg() }, true)
    } else {
        (fraction, false)
    };
    // `x` is finite and far from zero here, so its sign bit is its sign.
    if x.is_sign_negative() {
        negative = !negative;
        quadrant = 4_u128.wrapping_sub(quadrant) & 3;
    }
    let q = match quadrant {
        1 => 1.0,
        2 => 2.0,
        3 => -1.0,
        _ => 0.0,
    };
    let length = magnitude.bit_length();
    if length == 0 {
        return (0.0, 0.0, q);
    }
    let top = if length >= KEPT {
        magnitude.shr_low(length.wrapping_sub(KEPT))
    } else {
        magnitude.lo.checked_shl(KEPT.wrapping_sub(length)).unwrap_or(0)
    };
    let scale = i32::try_from(length)
        .unwrap_or(0)
        .wrapping_sub(i32::try_from(KEPT).unwrap_or(0))
        .wrapping_sub(i32::try_from(point).unwrap_or(0));
    let f_hi = (top >> 64) as f64 * power_of_two(scale.wrapping_add(64));
    let f_lo = u64::try_from(top & u128::from(u64::MAX)).unwrap_or(0) as f64 * power_of_two(scale);
    let (f_hi, f_lo) = if negative { (-f_hi, -f_lo) } else { (f_hi, f_lo) };
    let (p, error) = two_product(f_hi, PIO2_HI);
    let (r, rr) = fast_two_sum(p, error + (f_hi * PIO2_LO + f_lo * PIO2_HI));
    (r, rr, q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_starts_at_the_bit_asked_for() {
        assert_eq!(window(1), [TWO_OVER_PI[0], TWO_OVER_PI[1], TWO_OVER_PI[2]]);
        assert_eq!(window(65), [TWO_OVER_PI[1], TWO_OVER_PI[2], TWO_OVER_PI[3]]);
        let shifted = window(5);
        assert_eq!(shifted[0], (TWO_OVER_PI[0] << 4) | (TWO_OVER_PI[1] >> 60));
        assert_eq!(shifted[2], (TWO_OVER_PI[2] << 4) | (TWO_OVER_PI[3] >> 60));
    }

    #[test]
    fn the_product_is_exact() {
        let wide = product(3, [1, 0, u64::MAX]);
        assert_eq!(wide, Wide { hi: 3, lo: (u128::from(u64::MAX)) * 3 });
        let carried = product((1 << 53) - 1, [u64::MAX; 3]);
        // (2^53 - 1)(2^192 - 1) = 2^245 - 2^192 - 2^53 + 1 = (2^117 - 2^64 - 1) 2^128 + (2^128 - 2^53 + 1).
        assert_eq!(carried.hi, (1_u128 << 117) - (1 << 64) - 1);
        assert_eq!(carried.lo, 1_u128.wrapping_sub(1 << 53));
    }

    #[test]
    fn shifting_and_lengths_cross_the_halves() {
        let wide = Wide { hi: 0b101, lo: 1 << 127 };
        assert_eq!(wide.bit_length(), 131);
        assert_eq!(wide.shr_low(127), 0b1011);
        assert_eq!(wide.shr_low(128), 0b101);
        assert_eq!(wide.shr_low(0), 1 << 127);
        assert_eq!(Wide { hi: 0, lo: 5 }.bit_length(), 3);
        assert_eq!(Wide { hi: 0, lo: 0 }.bit_length(), 0);
    }

    #[test]
    fn small_and_non_finite_arguments_are_left_alone() {
        assert_eq!(reduce(1.0), (0.0, 0.0, 0.0));
        assert_eq!(reduce(-LIMIT), (0.0, 0.0, 0.0));
        let (r, rr, q) = reduce(f64::INFINITY);
        assert!(r.is_nan() && rr.is_nan() && q == 0.0);
        assert!(reduce(f64::NAN).0.is_nan());
    }

    #[test]
    fn a_large_multiple_of_a_quarter_turn_reduces_near_zero() {
        // 2^21 * (π/2 rounded) is within a few ulps of a multiple of π/2, in quadrant 0 since 2^21 is a multiple of 4.
        let x = PIO2_HI * 2_097_152.0;
        let (r, rr, q) = reduce(x);
        assert_eq!(q, 0.0);
        assert!(r.abs() < 1e-9 && rr.abs() <= r.abs() * f64::EPSILON, "{r} {rr}");
        assert_eq!(reduce(-x), (-r, -rr, 0.0));
        let (_, _, q_odd) = reduce(PIO2_HI * 2_097_153.0);
        assert_eq!(q_odd, 1.0);
        let (_, _, q_back) = reduce(-PIO2_HI * 2_097_153.0);
        assert_eq!(q_back, -1.0);
    }

    #[test]
    fn the_largest_double_reduces_into_range() {
        let (r, rr, q) = reduce(f64::MAX);
        assert!(r.abs() <= 0.785_398_163_397_448_4 && rr.abs() <= r.abs() * f64::EPSILON, "{r}");
        assert!([0.0, 1.0, 2.0, -1.0].contains(&q));
    }
}
