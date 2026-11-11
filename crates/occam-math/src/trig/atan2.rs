//! Source: original
//! The owned `atan2`: the octant from comparisons, `atan` of a quotient within a quarter of an anchor at 0, 1/2 or 1,
//! and the pieces added as a double-double so that one rounding comes last. Within about 1.5 ulps (S5), with C99's
//! special cases.

use super::constants::{
    ATAN, ATAN_HALF_HI, ATAN_HALF_LO, PI_HI, PI_LO, PIO2_HI, PIO2_LO, PIO4_HI, PIO4_LO, THREE_PIO4,
};
use crate::exact::two_sum;
use crate::real::Real;

/// 2^-1000 and 2^100: a larger part below the first is scaled up by the second, so that nothing is subnormal.
const SMALL: f64 = f64::from_bits(0x0170_0000_0000_0000);
const UP: f64 = f64::from_bits(0x4630_0000_0000_0000);

/// 2^1021: a larger part from here on is halved, so that the denominator cannot overflow.
const LARGE: f64 = f64::from_bits(0x7FC0_0000_0000_0000);

/// The angle of the point `(x, y)` from the positive x axis, from -π to π: the same bits on every target, lane width and
/// build, within about 1.5 ulps. Follows C99's special cases: the sign of `y` is kept, zero included, `atan2(±0, -0)`
/// is ±π, and two infinities give ±π/4 or ±3π/4. NaN if either is NaN.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn atan2<R: Real>(y: R, x: R) -> R {
    let splat = R::splat;
    let (zero, one) = (splat(0.0), splat(1.0));
    let (size_x, size_y) = (x.abs(), y.abs());
    let x_negative = one.copysign(x).lt(zero);
    let swap = size_x.lt(size_y);
    let (smaller, larger) = (R::select(swap, size_x, size_y), R::select(swap, size_y, size_x));
    let scale = R::select(larger.lt(splat(SMALL)), splat(UP), R::select(splat(LARGE).le(larger), splat(0.5), one));
    let (smaller, larger) = (smaller * scale, larger * scale);
    let near_zero = smaller.lt(splat(0.25) * larger);
    let near_half = smaller.lt(splat(0.75) * larger);
    let anchor = R::select(near_zero, zero, R::select(near_half, splat(0.5), one));
    let theta_hi = R::select(near_zero, zero, R::select(near_half, splat(ATAN_HALF_HI), splat(PIO4_HI)));
    let theta_lo = R::select(near_zero, zero, R::select(near_half, splat(ATAN_HALF_LO), splat(PIO4_LO)));
    // Selected, not multiplied by an anchor of 0, so that `0 * inf` cannot make a finite over an infinite NaN.
    let ratio = R::select(near_zero, smaller, smaller - anchor * larger)
        / R::select(near_zero, larger, larger + anchor * smaller);
    let square = ratio * ratio;
    let polynomial = ATAN.iter().rev().fold(zero, |sum, coefficient| splat(*coefficient) + square * sum);
    let atan = ratio + (ratio * square) * polynomial;
    let offset_hi = R::select(swap, splat(PIO2_HI), R::select(x_negative, splat(PI_HI), zero));
    let offset_lo = R::select(swap, splat(PIO2_LO), R::select(x_negative, splat(PI_LO), zero));
    let sign = R::select(x_negative, -one, one) * R::select(swap, -one, one);
    let (high, error) = two_sum(offset_hi, sign * theta_hi);
    let angle = high + (error + (offset_lo + sign * (theta_lo + atan)));
    let infinity = splat(f64::INFINITY);
    let both_zero = R::select(larger.le(zero), smaller, one).le(zero);
    let both_infinite = infinity.le(R::select(infinity.le(larger), smaller, zero));
    let angle = R::select(both_zero, R::select(x_negative, splat(PI_HI), zero), angle);
    let angle = R::select(both_infinite, R::select(x_negative, splat(THREE_PIO4), splat(PIO4_HI)), angle);
    angle.copysign(y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::canonical_bits;
    use crate::lanes::Lanes;
    use crate::splitmix::SplitMix64;
    use crate::testing::double;

    fn bits(x: f64) -> u64 {
        canonical_bits(x)
    }

    /// 2^`k`, by doubling or halving one at a time.
    fn power(k: i32) -> f64 {
        let step = if k < 0 { 0.5 } else { 2.0 };
        (0..k.unsigned_abs()).fold(1.0, |x, _| x * step)
    }

    #[test]
    fn the_constants_are_the_powers_of_two_they_claim() {
        assert_eq!(SMALL, power(-1000));
        assert_eq!(UP, power(100));
        assert_eq!(LARGE, power(1021));
    }

    #[test]
    fn signed_zeros_follow_c99() {
        assert_eq!(bits(atan2(0.0, 0.0)), bits(0.0));
        assert_eq!(bits(atan2(-0.0, 0.0)), bits(-0.0));
        assert_eq!(atan2(0.0, -0.0), PI_HI);
        assert_eq!(atan2(-0.0, -0.0), -PI_HI);
        assert_eq!(atan2(0.0, -3.0), PI_HI);
        assert_eq!(atan2(-0.0, -3.0), -PI_HI);
        assert_eq!(bits(atan2(-0.0, 3.0)), bits(-0.0));
        assert_eq!(atan2(2.0, 0.0), PIO2_HI);
        assert_eq!(atan2(-2.0, -0.0), -PIO2_HI);
    }

    #[test]
    fn infinities_follow_c99() {
        let infinity = f64::INFINITY;
        assert_eq!(bits(atan2(1.0, infinity)), bits(0.0));
        assert_eq!(bits(atan2(-1.0, infinity)), bits(-0.0));
        assert_eq!(atan2(1.0, -infinity), PI_HI);
        assert_eq!(atan2(-1.0, -infinity), -PI_HI);
        assert_eq!(atan2(infinity, 5.0), PIO2_HI);
        assert_eq!(atan2(-infinity, -5.0), -PIO2_HI);
        assert_eq!(atan2(infinity, infinity), PIO4_HI);
        assert_eq!(atan2(-infinity, infinity), -PIO4_HI);
        assert_eq!(atan2(infinity, -infinity), THREE_PIO4);
        assert_eq!(atan2(-infinity, -infinity), -THREE_PIO4);
    }

    #[test]
    fn nan_in_either_gives_nan_even_beside_a_zero_or_an_infinity() {
        for (y, x) in [(f64::NAN, 1.0), (1.0, f64::NAN), (f64::NAN, 0.0), (0.0, f64::NAN), (f64::INFINITY, f64::NAN)] {
            assert!(atan2(y, x).is_nan(), "atan2({y}, {x})");
        }
        assert!(atan2(f64::NAN, f64::INFINITY).is_nan());
    }

    #[test]
    fn the_axes_and_diagonals_are_the_rounded_constants() {
        assert_eq!(atan2(1.0, 1.0), PIO4_HI);
        assert_eq!(atan2(-1.0, -1.0), -THREE_PIO4);
        assert_eq!(atan2(1.0, 2.0), ATAN_HALF_HI);
        assert_eq!(bits(atan2(0.0, 1.0)), bits(0.0));
    }

    #[test]
    fn extreme_sizes_neither_overflow_nor_underflow() {
        let tiny = f64::from_bits(3);
        assert_eq!(atan2(tiny, tiny), PIO4_HI);
        assert_eq!(atan2(f64::MAX, f64::MAX), PIO4_HI);
        assert!((atan2(f64::MAX, f64::MAX / 2.0) - 1.107_148_717_794_090_4).abs() <= 2.0 * f64::EPSILON);
        assert!((atan2(tiny, f64::from_bits(4)) - 0.643_501_108_793_284_4).abs() <= 2.0 * f64::EPSILON);
    }

    #[test]
    fn it_is_odd_in_y_and_turns_with_the_point() {
        let mut generator = SplitMix64::new(61);
        for _ in 0..20_000 {
            let (y, x) = (double(&mut generator, -40, 40), double(&mut generator, -40, 40));
            assert_eq!(bits(atan2(-y, x)), bits(-atan2(y, x)), "{y} {x}");
            let angle = atan2(y, x);
            assert!(angle.abs() <= PI_HI, "{y} {x}");
            assert_eq!(angle < 0.0, y < 0.0);
            // Beyond a quarter turn for x below zero; a point near the y axis may round to exactly π/2 either way.
            if x < 0.0 {
                assert!(angle.abs() >= PIO2_HI, "{y} {x}");
            } else {
                assert!(angle.abs() <= PIO2_HI, "{y} {x}");
            }
        }
    }

    #[test]
    fn lanes_give_each_lanes_bits() {
        let mut generator = SplitMix64::new(62);
        for _ in 0..2_000 {
            let pick = |generator: &mut SplitMix64| match generator.next_u64().checked_rem(6).unwrap() {
                0 => 0.0,
                1 => -0.0,
                2 => f64::INFINITY,
                3 => f64::NAN,
                _ => double(generator, -1000, 1000),
            };
            let ys: [f64; 4] = std::array::from_fn(|_| pick(&mut generator));
            let xs: [f64; 4] = std::array::from_fn(|_| pick(&mut generator));
            let angles = atan2(Lanes::new(ys), Lanes::new(xs)).to_array();
            for lane in 0..4 {
                assert_eq!(bits(angles[lane]), bits(atan2(ys[lane], xs[lane])), "{} {}", ys[lane], xs[lane]);
            }
        }
    }
}
