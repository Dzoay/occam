//! Source: shewchuk-1997
//! Error-free transformations: a sum or a product as a rounded result and its exact error, in plain IEEE arithmetic with
//! no fused multiply-add (rule 5), written once over [`Real`] so that they run in lanes too (Shewchuk 1997, after
//! Knuth, Dekker and Veltkamp).

use crate::real::Real;

/// 2^27 + 1: multiplying by it splits a double's 53 significant bits into two halves of at most 26 bits each.
const SPLITTER: f64 = 134_217_729.0;

/// `(s, e)` with `s = fl(a + b)` and `a + b = s + e` exactly, for any finite `a` and `b` whose sum does not overflow.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn two_sum<R: Real>(a: R, b: R) -> (R, R) {
    let s = a + b;
    let b_virtual = s - a;
    let a_virtual = s - b_virtual;
    let b_round = b - b_virtual;
    let a_round = a - a_virtual;
    (s, a_round + b_round)
}

/// As [`two_sum`], in three operations instead of six, when `|a| >= |b|` or `a` is zero.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn fast_two_sum<R: Real>(a: R, b: R) -> (R, R) {
    let s = a + b;
    (s, b - (s - a))
}

/// `(hi, lo)` with `a = hi + lo` exactly, each of at most 26 significant bits, for `|a|` below about 2^996.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn split<R: Real>(a: R) -> (R, R) {
    let c = R::splat(SPLITTER) * a;
    let big = c - a;
    let hi = c - big;
    (hi, a - hi)
}

/// `(p, e)` with `p = fl(a * b)` and `a * b = p + e` exactly, for `|a|` and `|b|` below about 2^996 whose product
/// neither overflows nor has an error below the smallest normal number times 2^53.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn two_product<R: Real>(a: R, b: R) -> (R, R) {
    let p = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let first = p - a_hi * b_hi;
    let second = first - a_lo * b_hi;
    let third = second - a_hi * b_lo;
    (p, a_lo * b_lo - third)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;
    use crate::splitmix::SplitMix64;
    use crate::testing::{double, scaled, sum_scaled};

    #[test]
    fn two_sum_gives_the_exact_error_of_a_sum() {
        let mut generator = SplitMix64::new(11);
        for _ in 0..10_000 {
            let (a, b) = (double(&mut generator, -30, 30), double(&mut generator, -30, 30));
            let (s, e) = two_sum(a, b);
            assert_eq!(s.to_bits(), (a + b).to_bits());
            assert_eq!(sum_scaled(&[s, e], 90), sum_scaled(&[a, b], 90), "{a} + {b}");
        }
    }

    #[test]
    fn fast_two_sum_agrees_when_the_first_is_larger() {
        let mut generator = SplitMix64::new(12);
        for _ in 0..10_000 {
            let (a, b) = (double(&mut generator, 0, 30), double(&mut generator, -30, -1));
            assert_eq!(fast_two_sum(a, b), two_sum(a, b));
        }
    }

    #[test]
    fn split_gives_two_halves_that_sum_exactly() {
        let mut generator = SplitMix64::new(13);
        for _ in 0..10_000 {
            let a = double(&mut generator, -20, 20);
            let (hi, lo) = split(a);
            assert_eq!(sum_scaled(&[hi, lo], 80), scaled(a, 80));
            for half in [hi, lo] {
                let trailing = half.to_bits().trailing_zeros().min(52);
                assert!(trailing >= 26 || half == 0.0, "{half} has at most 26 significant bits");
            }
        }
    }

    #[test]
    fn two_product_gives_the_exact_error_of_a_product() {
        let mut generator = SplitMix64::new(14);
        for _ in 0..10_000 {
            let (a, b) = (double(&mut generator, -3, 3), double(&mut generator, -3, 3));
            let (p, e) = two_product(a, b);
            assert_eq!(p.to_bits(), (a * b).to_bits());
            assert_eq!(sum_scaled(&[p, e], 112), scaled(a, 56).checked_mul(scaled(b, 56)).unwrap(), "{a} * {b}");
        }
    }

    #[test]
    fn they_give_the_same_bits_in_lanes() {
        let mut generator = SplitMix64::new(15);
        let first: [f64; 4] = std::array::from_fn(|_| double(&mut generator, -20, 20));
        let second: [f64; 4] = std::array::from_fn(|_| double(&mut generator, -20, 20));
        let (sum, sum_error) = two_sum(Lanes::new(first), Lanes::new(second));
        let (product, product_error) = two_product(Lanes::new(first), Lanes::new(second));
        for lane in 0..4 {
            let pair = (first[lane], second[lane]);
            assert_eq!((sum.to_array()[lane], sum_error.to_array()[lane]), two_sum(pair.0, pair.1));
            assert_eq!((product.to_array()[lane], product_error.to_array()[lane]), two_product(pair.0, pair.1));
        }
    }
}
