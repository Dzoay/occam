//! Source: original
//! [`sinh_cosh`]: the hyperbolic sine and cosine on the owned [`exp`] and [`expm1`], so that a hyperbola keeps its usual
//! parameter (`docs/decisions/2026-11-11-exp-from-arm.md`).

use crate::exp::{exp, expm1};
use crate::real::Real;

/// From here on in magnitude, e^|x| overflows before it is halved, so both come from e^(|x|/2) instead.
const OVERFLOWING: f64 = 709.0;

/// 18!, the largest factorial a double holds exactly.
const F18: f64 = 6_402_373_705_728_000.0;

/// Taylor's coefficients 1/n! of sinh's odd powers from x³ to x¹⁹, 19! rounded once. Below 1 the terms left out are
/// under 2^-64 of the sum.
const SINH: [f64; 9] = [
    1.0 / 6.0,
    1.0 / 120.0,
    1.0 / 5040.0,
    1.0 / 362_880.0,
    1.0 / 39_916_800.0,
    1.0 / 6_227_020_800.0,
    1.0 / 1_307_674_368_000.0,
    1.0 / 355_687_428_096_000.0,
    1.0 / (F18 * 19.0),
];

/// Taylor's coefficients 1/n! of cosh's even powers from x² to x¹⁸. Below 1 the terms left out are under 2^-61 of the
/// sum.
const COSH: [f64; 9] = [
    1.0 / 2.0,
    1.0 / 24.0,
    1.0 / 720.0,
    1.0 / 40_320.0,
    1.0 / 3_628_800.0,
    1.0 / 479_001_600.0,
    1.0 / 87_178_291_200.0,
    1.0 / 20_922_789_888_000.0,
    1.0 / F18,
];

/// `p[0] + z (p[1] + z (... + z p[8]))`.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn horner<R: Real>(z: R, p: &[f64]) -> R {
    p.iter().rev().fold(R::splat(0.0), |sum, coefficient| R::splat(*coefficient) + z * sum)
}

/// `(sinh x, cosh x)`: the same bits on every target, lane width and build. Below 1 in magnitude, Taylor's series,
/// whose terms are all positive, so nothing cancels. From 1, with E = e^|x| − 1 from [`expm1`] and e = E + 1,
/// sinh |x| = (E + E/e)/2 and cosh |x| = (e + 1/e)/2. From 709, both are (e^(|x|/2)/2) e^(|x|/2), which reaches the
/// largest double before it overflows. The sine takes `x`'s sign, zero included. NaN for NaN.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn sinh_cosh<R: Real>(x: R) -> (R, R) {
    let (half, one) = (R::splat(0.5), R::splat(1.0));
    let size = x.abs();
    let z = size * size;
    let below = size.lt(one);
    let e_less_one = expm1(size);
    let e = e_less_one + one;
    let sinh = R::select(below, size + (size * z) * horner(z, &SINH), half * (e_less_one + e_less_one / e));
    let cosh = R::select(below, one + z * horner(z, &COSH), half * (e + one / e));
    let root = exp(half * size);
    let large = (half * root) * root;
    let far = R::splat(OVERFLOWING).le(size);
    (R::select(far, large, sinh).copysign(x), R::select(far, large, cosh))
}

/// `sinh x`: exactly [`sinh_cosh`]'s first part.
#[inline(always)]
pub fn sinh<R: Real>(x: R) -> R {
    sinh_cosh(x).0
}

/// `cosh x`: exactly [`sinh_cosh`]'s second part.
#[inline(always)]
pub fn cosh<R: Real>(x: R) -> R {
    sinh_cosh(x).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;

    fn apart(a: f64, b: f64) -> u64 {
        a.to_bits().abs_diff(b.to_bits())
    }

    #[test]
    fn sinh_and_cosh_are_within_an_ulp() {
        // From Python's `decimal` at 60 digits, rounded once to a double.
        let known = [
            (1.0e-10, 1.0e-10, 1.0),
            (1.0e-05, 1.000_000_000_016_666_8e-05, 1.000_000_000_05),
            (0.5, 0.521_095_305_493_747_4, 1.127_625_965_206_380_7),
            (1.0, 1.175_201_193_643_801_4, 1.543_080_634_815_243_7),
            (-2.0, -3.626_860_407_847_018_6, 3.762_195_691_083_631_4),
            (20.0, 242_582_597.704_895_14, 242_582_597.704_895_14),
            (30.0, 5_343_237_290_762.231, 5_343_237_290_762.231),
            (-300.0, -9.712_131_976_206_279e+129, 9.712_131_976_206_279e+129),
            (709.5, 6.774_931_596_573_164e+307, 6.774_931_596_573_164e+307),
            (710.0, 1.116_997_383_080_855_5e+308, 1.116_997_383_080_855_5e+308),
        ];
        for (x, want_sinh, want_cosh) in known {
            let (s, c) = sinh_cosh(x);
            let ulps = if x.abs() < OVERFLOWING { 1 } else { 2 };
            assert!(apart(s, want_sinh) <= ulps && apart(c, want_cosh) <= ulps, "{x}: {s} {c}");
            assert_eq!((sinh(x).to_bits(), cosh(x).to_bits()), (s.to_bits(), c.to_bits()));
        }
    }

    #[test]
    fn sinh_is_odd_and_cosh_even() {
        for x in [1e-300, 1e-8, 0.3, 0.999, 1.0, 1.001, 5.0, 25.0, 708.9, 709.0, 709.9] {
            let ((s, c), (ns, nc)) = (sinh_cosh(x), sinh_cosh(-x));
            assert_eq!((ns.to_bits(), nc.to_bits()), ((-s).to_bits(), c.to_bits()), "{x}");
            assert!(s > 0.0 && c >= 1.0, "{x}: {s} {c}");
        }
    }

    #[test]
    fn the_forms_meet_where_they_change() {
        for edge in [1.0, OVERFLOWING] {
            let (below, at) = (sinh_cosh(edge - 1e-9), sinh_cosh(edge));
            let gap = |a: f64, b: f64| (b - a) / a;
            assert!(gap(below.0, at.0) > 0.0 && gap(below.0, at.0) < 2e-9, "sinh at {edge}: {below:?} {at:?}");
            assert!(gap(below.1, at.1) > 0.0 && gap(below.1, at.1) < 2e-9, "cosh at {edge}: {below:?} {at:?}");
        }
        let (s, c) = sinh_cosh(0.999_999_999);
        assert!(apart(c * c - s * s, 1.0) <= 8, "{s} {c}");
    }

    #[test]
    fn sinh_and_cosh_meet_their_limits() {
        assert_eq!(sinh_cosh(-0.0).0.to_bits(), (-0.0f64).to_bits());
        assert_eq!(sinh_cosh(0.0), (0.0, 1.0));
        assert_eq!(sinh_cosh(f64::NEG_INFINITY), (f64::NEG_INFINITY, f64::INFINITY));
        assert_eq!(sinh_cosh(711.0), (f64::INFINITY, f64::INFINITY));
        assert!(sinh_cosh(710.4).0.is_finite());
        let (s, c) = sinh_cosh(f64::NAN);
        assert!(s.is_nan() && c.is_nan());
    }

    #[test]
    fn lanes_give_each_lanes_own_bits() {
        let x = [0.5, -2.0, 709.5, 1e-10, f64::NAN, -0.0, 0.999, -711.0];
        let (s, c) = sinh_cosh(Lanes::new(x));
        let bits = |v: [f64; 8]| v.map(crate::canonical_bits);
        assert_eq!(bits(s.to_array()), bits(x.map(sinh)));
        assert_eq!(bits(c.to_array()), bits(x.map(cosh)));
    }
}
