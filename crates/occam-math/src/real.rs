//! Source: original
//! [`Real`]: the number type the kernel's maths is written over, once.

use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::lanes::Lanes;
use crate::mask::Mask;

mod sealed {
    use super::Lanes;

    /// Only this crate implements [`Real`](super::Real), so every implementation is under the determinism contract and
    /// the trait can grow without breaking anyone.
    pub trait Sealed {}
    impl Sealed for f64 {}
    impl<const W: usize> Sealed for Lanes<W> {}
}

/// A number, or several side by side. A formula written once over `Real` gives on lane `k` exactly the bits it gives on
/// `f64` for that lane's inputs.
///
/// Every operation here is correctly rounded by IEEE 754, so its result is the same on every target. `docs/DETERMINISM.md`
/// says what else may join them.
pub trait Real:
    sealed::Sealed
    + Copy
    + Debug
    + Send
    + Sync
    + 'static
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// How many numbers one value holds.
    const LANES: usize;

    /// Which lanes a comparison held in.
    type Mask: Mask;

    /// A value holding `x` in every lane.
    fn splat(x: f64) -> Self;

    /// A value whose lanes are `next`'s results, lane 0 first.
    fn from_fn(next: impl FnMut() -> f64) -> Self;

    /// Calls `visit` on every lane, lane 0 first.
    fn for_each_lane(self, visit: impl FnMut(f64));

    /// The absolute value.
    #[must_use]
    fn abs(self) -> Self;

    /// The square root: NaN below zero, and -0 for -0.
    #[must_use]
    fn sqrt(self) -> Self;

    /// The lanes where `self` is less than `rhs`. Exact, and false where either side is NaN. The two zeros are equal.
    fn lt(self, rhs: Self) -> Self::Mask;

    /// The lanes where `self` is at most `rhs`. Exact, and false where either side is NaN. The two zeros are equal.
    fn le(self, rhs: Self) -> Self::Mask;

    /// `yes` in the lanes where `mask` holds and `no` in the others, bits unchanged. This is the way to a minimum or a
    /// maximum: `f64::min` stays banned, because which zero it returns differs by target.
    #[must_use]
    fn select(mask: Self::Mask, yes: Self, no: Self) -> Self;
}

impl Real for f64 {
    const LANES: usize = 1;

    type Mask = bool;

    #[inline(always)]
    fn splat(x: f64) -> Self {
        x
    }

    #[inline(always)]
    fn from_fn(mut next: impl FnMut() -> f64) -> Self {
        next()
    }

    #[inline(always)]
    fn for_each_lane(self, mut visit: impl FnMut(f64)) {
        visit(self);
    }

    #[inline(always)]
    fn abs(self) -> Self {
        f64::abs(self)
    }

    #[inline(always)]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }

    #[inline(always)]
    fn lt(self, rhs: Self) -> bool {
        self < rhs
    }

    #[inline(always)]
    fn le(self, rhs: Self) -> bool {
        self <= rhs
    }

    #[inline(always)]
    fn select(mask: bool, yes: Self, no: Self) -> Self {
        if mask { yes } else { no }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scalar_holds_one_number() {
        assert_eq!(f64::LANES, 1);
        assert_eq!(<f64 as Real>::splat(2.5), 2.5);
        let mut seen = Vec::new();
        <f64 as Real>::from_fn(|| 7.5).for_each_lane(|x| seen.push(x));
        assert_eq!(seen, [7.5]);
    }

    #[test]
    fn the_absolute_value_clears_only_the_sign() {
        assert_eq!(<f64 as Real>::abs(-2.5), 2.5);
        assert_eq!(<f64 as Real>::abs(2.5), 2.5);
        assert_eq!(<f64 as Real>::abs(-0.0).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn comparisons_are_false_where_either_side_is_nan() {
        for (a, b) in [(f64::NAN, 1.0), (1.0, f64::NAN), (f64::NAN, f64::NAN), (f64::NAN, f64::INFINITY)] {
            assert!(!Real::lt(a, b), "{a} < {b}");
            assert!(!Real::le(a, b), "{a} <= {b}");
        }
    }

    #[test]
    fn comparisons_are_exact_and_the_zeros_are_equal() {
        assert!(Real::lt(1.0, 1.0 + f64::EPSILON));
        assert!(!Real::lt(1.0, 1.0));
        assert!(Real::le(1.0, 1.0));
        assert!(!Real::le(1.0 + f64::EPSILON, 1.0));
        assert!(Real::lt(f64::NEG_INFINITY, f64::MIN));
        assert!(!Real::lt(-0.0, 0.0));
        assert!(Real::le(-0.0, 0.0) && Real::le(0.0, -0.0));
    }

    #[test]
    fn select_takes_one_side_with_its_bits_unchanged() {
        assert_eq!(f64::select(true, -0.0, 0.0).to_bits(), (-0.0_f64).to_bits());
        assert_eq!(f64::select(false, -0.0, 0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(f64::select(true, 2.5, f64::NAN), 2.5);
        assert!(f64::select(false, 2.5, f64::NAN).is_nan());
    }

    #[test]
    fn the_square_root_is_correctly_rounded() {
        assert_eq!(<f64 as Real>::sqrt(2.0).to_bits(), 0x3FF6_A09E_667F_3BCD);
        assert_eq!(<f64 as Real>::sqrt(-0.0).to_bits(), (-0.0_f64).to_bits());
        assert!(<f64 as Real>::sqrt(-1.0).is_nan());
    }
}
