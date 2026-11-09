//! Source: original
//! [`Real`]: the number type the kernel's maths is written over, once.

use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::lanes::Lanes;

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
}

impl Real for f64 {
    const LANES: usize = 1;

    #[inline]
    fn splat(x: f64) -> Self {
        x
    }

    #[inline]
    fn from_fn(mut next: impl FnMut() -> f64) -> Self {
        next()
    }

    #[inline]
    fn for_each_lane(self, mut visit: impl FnMut(f64)) {
        visit(self);
    }

    #[inline]
    fn abs(self) -> Self {
        f64::abs(self)
    }

    #[inline]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
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
    fn the_square_root_is_correctly_rounded() {
        assert_eq!(<f64 as Real>::sqrt(2.0).to_bits(), 0x3FF6_A09E_667F_3BCD);
        assert_eq!(<f64 as Real>::sqrt(-0.0).to_bits(), (-0.0_f64).to_bits());
        assert!(<f64 as Real>::sqrt(-1.0).is_nan());
    }
}
