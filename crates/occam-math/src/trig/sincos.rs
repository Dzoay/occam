//! Source: cody-waite-1980
//! The owned `sin_cos`: Cody and Waite's reduction by π/2 in four pieces, Payne and Hanek's above 2^20, and kernels
//! fitted on |r| ≤ 0.786 by `tools/coefficients/fit.py`. Within about 0.75 ulps (S5).

use super::constants::{COS, INV_PIO2, PIO2_C1, PIO2_C2, PIO2_C3, PIO2_C4, SIN};
use super::large::{self, LIMIT};
use crate::exact::{two_product, two_sum};
use crate::mask::Mask;
use crate::real::Real;

/// 1.5 * 2^52: adding and subtracting it rounds a double below 2^51 to an integer, in plain arithmetic.
const MAGIC: f64 = 6_755_399_441_055_744.0;

/// 2^-27: below this, `sin x` rounds to `x` and `cos x` to 1.
const TINY: f64 = 1.0 / 134_217_728.0;

/// `x` rounded to the nearest integer, ties to even, for `|x|` below 2^51.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn nearest<R: Real>(x: R) -> R {
    (x + R::splat(MAGIC)) - R::splat(MAGIC)
}

/// `p[0] + z (p[1] + z (... + z p[5]))`.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn horner<R: Real>(z: R, p: &[f64]) -> R {
    p.iter().rev().fold(R::splat(0.0), |sum, coefficient| R::splat(*coefficient) + z * sum)
}

/// Cody and Waite's reduction of `|x|` up to [`LIMIT`]: `(r, rr, q)` with `x = k π/2 + r + rr` and `q = k` mod 4, from
/// -2 to 2. `k` times each of the first three pieces of π/2 is exact, and so is the first subtraction.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn reduce<R: Real>(x: R) -> (R, R, R) {
    let k = nearest(x * R::splat(INV_PIO2));
    let hi = x - k * R::splat(PIO2_C1);
    let (first, error) = two_sum(hi, -(k * R::splat(PIO2_C2)));
    let (second, more) = two_sum(first, -(k * R::splat(PIO2_C3)));
    let low = (error + more) - k * R::splat(PIO2_C4);
    let (r, rr) = two_sum(second, low);
    let q = k - R::splat(4.0) * nearest(k * R::splat(0.25));
    (r, rr, q)
}

/// The lanes past [`LIMIT`], or not finite, reduced by Payne and Hanek's method one at a time, the rest left as they are.
#[inline(always)]
fn reduce_large<R: Real>(x: R, fast: R::Mask, (r, rr, q): (R, R, R)) -> (R, R, R) {
    if fast.all() {
        return (r, rr, q);
    }
    let mut reduced = Vec::with_capacity(R::LANES);
    x.for_each_lane(|lane| reduced.push(large::reduce(lane)));
    let part = |pick: fn(&(f64, f64, f64)) -> f64| {
        let mut lanes = reduced.iter();
        R::from_fn(|| lanes.next().map_or(0.0, pick))
    };
    (
        R::select(fast, r, part(|lane| lane.0)),
        R::select(fast, rr, part(|lane| lane.1)),
        R::select(fast, q, part(|lane| lane.2)),
    )
}

/// `(sin r, cos r)` for `r + rr` with `|r|` at most about π/4.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn kernels<R: Real>(r: R, rr: R) -> (R, R) {
    let (one, half) = (R::splat(1.0), R::splat(0.5));
    let (z, z_error) = two_product(r, r);
    let sin = r + ((r * z) * horner(z, &SIN) + rr * (one - half * z));
    let h = half * z;
    let w = one - h;
    let error = (one - w) - h;
    // Half of `z`'s rounding error, taken off: up to a quarter of an ulp of the cosine near |r| = π/4.
    let cos = w + ((error - half * z_error) + ((z * z) * horner(z, &COS) - r * rr));
    (sin, cos)
}

/// `(sin x, cos x)`, the same bits on every target, lane width and build, within about 0.75 ulps. The sign of a zero is
/// kept for the sine. NaN for NaN and the infinities.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub fn sin_cos<R: Real>(x: R) -> (R, R) {
    let size = x.abs();
    let fast = size.le(R::splat(LIMIT));
    let (r, rr, q) = reduce_large(x, fast, reduce(x));
    let (sin_r, cos_r) = kernels(r, rr);
    let one = R::splat(1.0);
    let half = R::splat(0.5);
    let swap = ((q.abs() - one).abs()).lt(half);
    let sin_negative = one.lt((q - half).abs());
    let cos_negative = one.lt((q + half).abs());
    let (sin, cos) = (R::select(swap, cos_r, sin_r), R::select(swap, sin_r, cos_r));
    let (sin, cos) = (R::select(sin_negative, -sin, sin), R::select(cos_negative, -cos, cos));
    let small = size.lt(R::splat(TINY));
    (R::select(small, x, sin), R::select(small, one, cos))
}

/// `sin x`: exactly [`sin_cos`]'s first part.
#[inline(always)]
pub fn sin<R: Real>(x: R) -> R {
    sin_cos(x).0
}

/// `cos x`: exactly [`sin_cos`]'s second part.
#[inline(always)]
pub fn cos<R: Real>(x: R) -> R {
    sin_cos(x).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::canonical_bits;
    use crate::lanes::Lanes;
    use crate::splitmix::SplitMix64;
    use crate::testing::double;

    /// How many units in the last place `got` is from `want`, both near each other.
    fn ulps(got: f64, want: f64) -> f64 {
        let unit = want.abs().next_up() - want.abs();
        (got - want).abs() / unit
    }

    #[test]
    fn zeros_keep_their_sign_and_tiny_arguments_are_their_own_sine() {
        assert_eq!(sin_cos(0.0_f64), (0.0, 1.0));
        let (s, c) = sin_cos(-0.0_f64);
        assert_eq!((s.to_bits(), c), ((-0.0_f64).to_bits(), 1.0));
        for x in [1e-9, -3e-12, f64::MIN_POSITIVE, -f64::from_bits(1), TINY / 2.0] {
            assert_eq!(sin_cos(x), (x, 1.0), "{x}");
        }
    }

    #[test]
    fn nan_and_the_infinities_give_nan() {
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let (s, c) = sin_cos(x);
            assert!(s.is_nan() && c.is_nan(), "{x}");
        }
    }

    #[test]
    fn known_values_are_within_an_ulp() {
        // sin 1 = 0.84147098480789650665..., cos 1 = 0.54030230586813971740...
        let (s, c) = sin_cos(1.0_f64);
        assert!(ulps(s, 0.841_470_984_807_896_5) <= 0.75 && ulps(c, 0.540_302_305_868_139_8) <= 0.75, "{s} {c}");
        // sin 10^22 = -0.85220084976718880177..., which needs the large reduction.
        assert!(ulps(sin(1e22_f64), -0.852_200_849_767_188_8) <= 0.75, "{}", sin(1e22_f64));
        let (s, c) = sin_cos(core::f64::consts::FRAC_PI_6);
        assert!(ulps(s, 0.5) <= 1.0 && ulps(c, 0.866_025_403_784_438_7) <= 1.0, "{s} {c}");
    }

    #[test]
    fn sine_is_odd_and_cosine_even_at_every_size() {
        let mut generator = SplitMix64::new(51);
        for _ in 0..20_000 {
            let x = double(&mut generator, -30, 1000).abs();
            let ((s, c), (s_neg, c_neg)) = (sin_cos(x), sin_cos(-x));
            assert_eq!(canonical_bits(s_neg), canonical_bits(-s), "{x}");
            assert_eq!(canonical_bits(c_neg), canonical_bits(c), "{x}");
        }
    }

    #[test]
    fn the_square_of_sine_and_cosine_sums_to_one() {
        let mut generator = SplitMix64::new(52);
        for _ in 0..20_000 {
            let x = double(&mut generator, -10, 60);
            let (s, c) = sin_cos(x);
            assert!((s * s + c * c - 1.0).abs() <= 4.0 * f64::EPSILON, "{x}: {s} {c}");
            assert!(s.abs() <= 1.0 && c.abs() <= 1.0);
        }
    }

    #[test]
    fn the_two_reductions_meet_at_the_limit() {
        for x in [LIMIT.next_down(), LIMIT, -LIMIT, 1000.0, -0.75] {
            let (r, rr, q) = reduce(x);
            assert!(r.abs() <= 0.786 && rr.abs() <= r.abs() * f64::EPSILON && q.abs() <= 2.0, "{x}");
        }
        for x in [LIMIT.next_up(), -LIMIT.next_up()] {
            let (r, rr, q) = reduce(x);
            let (big_r, big_rr, big_q) = large::reduce(x);
            assert_eq!(q, big_q, "{x}");
            assert!(ulps(r + rr, big_r + big_rr) <= 1.0, "{x}: {r} {big_r}");
        }
    }

    #[test]
    fn lanes_give_each_lanes_bits_including_large_and_special_lanes() {
        let mut generator = SplitMix64::new(53);
        for _ in 0..2_000 {
            let lanes: [f64; 4] = std::array::from_fn(|_| match generator.next_u64().checked_rem(8).unwrap() {
                0 => f64::NAN,
                1 => 1e300 * double(&mut generator, 0, 0),
                2 => -0.0,
                _ => double(&mut generator, -30, 40),
            });
            let (s, c) = sin_cos(Lanes::new(lanes));
            for (lane, x) in lanes.iter().enumerate() {
                let (one_s, one_c) = sin_cos(*x);
                assert_eq!(canonical_bits(s.to_array()[lane]), canonical_bits(one_s), "{x}");
                assert_eq!(canonical_bits(c.to_array()[lane]), canonical_bits(one_c), "{x}");
            }
        }
    }

    #[test]
    fn sin_and_cos_are_the_two_halves() {
        for x in [0.3_f64, -2.0, 1e7, 5e-30] {
            assert_eq!((sin(x), cos(x)), sin_cos(x));
        }
    }
}
