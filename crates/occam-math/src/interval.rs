//! Source: rump-1999
//! [`Interval`]: a value and a certified radius around it, for decisions on computed values (S2). Rump's midpoint and
//! radius arithmetic, made outward without rounding modes: a rounded bound is stepped to the next double, which needs
//! only `+ - * /`, `sqrt` and [`Real::next_up`], so it gives the same bits on every target and in lanes.

use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::real::Real;

/// 2^-53. A rounded result that is a normal number is within this fraction of itself of the exact one.
const HALF_ULP: f64 = 1.0 / 9_007_199_254_740_992.0;

/// The smallest subnormal, which bounds the error of a result too small to be normal.
const TINY: f64 = f64::from_bits(1);

/// At least `x`, for an `x` that was rounded to nearest: the next double above it.
#[inline(always)]
fn up<R: Real>(x: R) -> R {
    x.next_up()
}

/// At least how far the exact result of one operation can be from `rounded`, its rounded value.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn rounding<R: Real>(rounded: R) -> R {
    up(R::splat(HALF_ULP) * rounded.abs() + R::splat(TINY))
}

/// The larger of two non-negative bounds, or NaN if the first is NaN.
#[inline(always)]
fn larger<R: Real>(a: R, b: R) -> R {
    R::select(a.lt(b), b, a)
}

/// Every value within `radius` of `mid`. An operation on two intervals gives one holding the exact result of that
/// operation on any values they hold. A NaN anywhere, or an infinite radius, holds everything, so no decision is made
/// from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval<R: Real = f64> {
    mid: R,
    rad: R,
}

impl<R: Real> Interval<R> {
    /// Exactly `x`.
    #[inline(always)]
    pub fn exact(x: R) -> Self {
        Self { mid: x, rad: R::splat(0.0) }
    }

    /// Every value within `radius` of `mid`. The radius is taken as its absolute value.
    #[inline(always)]
    pub fn new(mid: R, radius: R) -> Self {
        Self { mid, rad: radius.abs() }
    }

    /// The value at its middle: what plain arithmetic would have computed.
    #[inline(always)]
    pub const fn mid(self) -> R {
        self.mid
    }

    /// The certified radius of a computed value (S2 item 3). It is not a tolerance, and obeys no growth rule.
    #[inline(always)]
    pub const fn radius(self) -> R {
        self.rad
    }

    /// At most the lowest value it holds.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub fn lower(self) -> R {
        (self.mid - self.rad).next_down()
    }

    /// At least the highest value it holds.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub fn upper(self) -> R {
        up(self.mid + self.rad)
    }

    /// The square root of every value it holds that is not below zero: NaN if it holds none.
    #[must_use]
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub fn sqrt(self) -> Self {
        let zero = R::splat(0.0);
        let high = up(self.upper().sqrt());
        let below = self.lower();
        let low = R::select(zero.lt(below), below.sqrt().next_down(), zero);
        let mid = R::select(zero.le(self.mid), self.mid.sqrt(), R::splat(0.5) * high);
        Self { mid, rad: larger(up(high - mid), up(mid - low)) }
    }
}

impl<R: Real> Add for Interval<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn add(self, rhs: Self) -> Self {
        let mid = self.mid + rhs.mid;
        Self { mid, rad: up(up(self.rad + rhs.rad) + rounding(mid)) }
    }
}

impl<R: Real> Sub for Interval<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn sub(self, rhs: Self) -> Self {
        let mid = self.mid - rhs.mid;
        Self { mid, rad: up(up(self.rad + rhs.rad) + rounding(mid)) }
    }
}

impl<R: Real> Mul for Interval<R> {
    type Output = Self;

    /// `|a| rb + |b| ra + ra rb` bounds how far the product of any two held values is from `a b`.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn mul(self, rhs: Self) -> Self {
        let mid = self.mid * rhs.mid;
        let cross = up(up(self.mid.abs() * rhs.rad) + up(rhs.mid.abs() * self.rad));
        Self { mid, rad: up(up(cross + up(self.rad * rhs.rad)) + rounding(mid)) }
    }
}

impl<R: Real> Div for Interval<R> {
    type Output = Self;

    /// `(ra |b| + |a| rb) / (|b| (|b| - rb))` bounds how far the quotient of any two held values is from `a / b`. A
    /// divisor that may be zero gives an infinite radius.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn div(self, rhs: Self) -> Self {
        let mid = self.mid / rhs.mid;
        let size = rhs.mid.abs();
        let least = (size * (size - rhs.rad).next_down()).next_down();
        let spread = up(up(up(self.rad * size) + up(self.mid.abs() * rhs.rad)) / least);
        let bounded = R::splat(0.0).lt(least);
        Self { mid, rad: R::select(bounded, up(spread + rounding(mid)), R::splat(f64::INFINITY)) }
    }
}

impl<R: Real> Neg for Interval<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn neg(self) -> Self {
        Self { mid: -self.mid, rad: self.rad }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;
    use crate::expansion::Expansion;
    use crate::hash::canonical_bits;
    use crate::lanes::Lanes;
    use crate::splitmix::SplitMix64;
    use crate::testing::double;

    fn of(x: f64) -> Expansion {
        Expansion::of(x)
    }

    /// Whether `value`, exactly, is within the interval.
    fn holds(interval: Interval, value: &Expansion) -> bool {
        let offset = value.difference(&of(interval.mid()));
        let above = offset.difference(&of(interval.radius())).sign();
        let below = offset.sum(&of(interval.radius())).sign();
        matches!(above, Some(Ordering::Less | Ordering::Equal))
            && matches!(below, Some(Ordering::Greater | Ordering::Equal))
    }

    /// An interval of moderate size: exact a quarter of the time, otherwise a radius from 2^-50 to 2^-5 of its middle.
    fn interval(generator: &mut SplitMix64) -> Interval {
        let mid = double(generator, -20, 20);
        if generator.next_u64().checked_rem(4) == Some(0) {
            return Interval::exact(mid);
        }
        let fraction = double(generator, -50, -5).abs();
        Interval::new(mid, mid * fraction)
    }

    /// A value the interval holds: its middle, an end, or a point between.
    fn inside(generator: &mut SplitMix64, interval: Interval) -> f64 {
        let t = [-1.0, -0.5, 0.0, 0.3, 1.0][usize::try_from(generator.next_u64().checked_rem(5).unwrap()).unwrap()];
        let point = interval.mid() + t * interval.radius();
        if holds(interval, &of(point)) { point } else { interval.mid() }
    }

    #[test]
    fn sums_differences_and_products_hold_the_exact_result() {
        let mut generator = SplitMix64::new(31);
        for _ in 0..20_000 {
            let (a, b) = (interval(&mut generator), interval(&mut generator));
            let (x, y) = (inside(&mut generator, a), inside(&mut generator, b));
            assert!(holds(a + b, &of(x).sum(&of(y))), "{a:?} + {b:?} at {x}, {y}");
            assert!(holds(a - b, &of(x).difference(&of(y))), "{a:?} - {b:?} at {x}, {y}");
            assert!(holds(a * b, &of(x).product(&of(y))), "{a:?} * {b:?} at {x}, {y}");
            assert!(holds(-a, &of(x).negated()));
        }
    }

    #[test]
    fn quotients_hold_the_exact_result() {
        let mut generator = SplitMix64::new(32);
        for _ in 0..20_000 {
            let (a, b) = (interval(&mut generator), interval(&mut generator));
            let (x, y) = (inside(&mut generator, a), inside(&mut generator, b));
            let quotient = a / b;
            // x / y lies within [mid - radius, mid + radius] when each end times y is on the right side of x.
            let lower = of(quotient.mid()).difference(&of(quotient.radius()));
            let upper = of(quotient.mid()).sum(&of(quotient.radius()));
            let side = |end: &Expansion| end.product(&of(y)).difference(&of(x)).sign();
            let (low, high) = if y > 0.0 { (side(&lower), side(&upper)) } else { (side(&upper), side(&lower)) };
            assert!(low != Some(Ordering::Greater) && high != Some(Ordering::Less), "{a:?} / {b:?} at {x}, {y}");
        }
    }

    #[test]
    fn square_roots_hold_the_exact_result() {
        let mut generator = SplitMix64::new(33);
        for _ in 0..20_000 {
            let a = interval(&mut generator);
            let a = Interval::new(a.mid().abs(), a.radius());
            let x = inside(&mut generator, a);
            if x < 0.0 {
                continue;
            }
            let root = a.sqrt();
            let lower = of(root.mid()).difference(&of(root.radius()));
            let upper = of(root.mid()).sum(&of(root.radius()));
            let square_minus_x = |end: &Expansion| end.product(end).difference(&of(x)).sign();
            let low_ok = lower.sign() != Some(Ordering::Greater) || square_minus_x(&lower) != Some(Ordering::Greater);
            let high_ok = upper.sign() != Some(Ordering::Less) && square_minus_x(&upper) != Some(Ordering::Less);
            assert!(low_ok && high_ok, "sqrt of {a:?} at {x}");
        }
    }

    #[test]
    fn an_exact_result_gets_a_radius_of_a_few_units_in_the_last_place() {
        let sum = Interval::exact(1.0) + Interval::exact(2.0);
        assert_eq!(sum.mid(), 3.0);
        assert!(sum.radius() > 0.0 && sum.radius() <= 4.0 * f64::EPSILON, "{sum:?}");
        let product = Interval::exact(3.0) * Interval::exact(1.0 / 3.0);
        assert!(product.radius() > 0.0 && product.radius() <= 2.0 * f64::EPSILON, "{product:?}");
        let quotient = Interval::exact(1.0) / Interval::exact(3.0);
        assert!(quotient.radius() > 0.0 && quotient.radius() <= f64::EPSILON, "{quotient:?}");
        let root = Interval::exact(2.0).sqrt();
        assert!(root.radius() > 0.0 && root.radius() <= 4.0 * f64::EPSILON, "{root:?}");
    }

    #[test]
    fn the_bounds_lie_outside_what_it_holds() {
        let a = Interval::new(1.0, 1e-20);
        assert!(a.lower() < 1.0 && a.upper() > 1.0);
        assert!(holds(Interval::new(a.lower(), 0.0), &of(1.0).difference(&of(1e-20))) || a.lower() < 1.0 - 1e-20);
        assert_eq!(Interval::exact(0.0).upper(), f64::from_bits(1));
        assert_eq!(Interval::exact(0.0).lower(), -f64::from_bits(1));
    }

    #[test]
    fn a_divisor_that_may_be_zero_holds_everything() {
        let quotient = Interval::exact(1.0) / Interval::new(0.5, 1.0);
        assert_eq!(quotient.radius(), f64::INFINITY);
        let touching = Interval::exact(1.0) / Interval::new(0.5, 0.5);
        assert_eq!(touching.radius(), f64::INFINITY);
    }

    #[test]
    fn the_root_of_a_negative_interval_is_nan_and_one_reaching_zero_starts_there() {
        let root = Interval::new(-4.0, 1.0).sqrt();
        assert!(root.mid().is_nan() || root.radius().is_nan());
        let straddling = Interval::new(-1.0, 4.0).sqrt();
        assert!(straddling.mid() - straddling.radius() <= 0.0, "{straddling:?} reaches zero");
        let top = of(straddling.mid()).sum(&of(straddling.radius()));
        assert_eq!(top.product(&top).difference(&of(3.0)).sign(), Some(Ordering::Greater), "{straddling:?} reaches 3");
    }

    #[test]
    fn nan_spreads_to_the_result() {
        let nan = Interval::exact(f64::NAN);
        for result in [nan + Interval::exact(1.0), Interval::exact(1.0) * nan, nan / Interval::exact(2.0), nan.sqrt()] {
            assert!(result.mid().is_nan() || result.radius().is_nan(), "{result:?}");
        }
    }

    #[test]
    fn lanes_give_each_lanes_bits() {
        let mut generator = SplitMix64::new(34);
        let a: [Interval; 4] = std::array::from_fn(|_| interval(&mut generator));
        let b: [Interval; 4] = std::array::from_fn(|_| interval(&mut generator));
        let lanes = |pick: &[Interval; 4]| {
            Interval::new(Lanes::new(pick.map(Interval::mid)), Lanes::new(pick.map(Interval::radius)))
        };
        let (la, lb) = (lanes(&a), lanes(&b));
        let results = [la + lb, la - lb, la * lb, la / lb, -la, la.sqrt()];
        for lane in 0..4 {
            let (x, y) = (a[lane], b[lane]);
            let scalar = [x + y, x - y, x * y, x / y, -x, x.sqrt()];
            for (lanes, one) in results.iter().zip(scalar) {
                assert_eq!(canonical_bits(lanes.mid().to_array()[lane]), canonical_bits(one.mid()));
                assert_eq!(canonical_bits(lanes.radius().to_array()[lane]), canonical_bits(one.radius()));
            }
        }
    }
}
