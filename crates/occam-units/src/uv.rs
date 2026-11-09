//! Source: original
//! Positions on a surface. The two parameters are plain numbers, since what they measure depends on the surface: on a plane
//! both are lengths, and on a cylinder one is an angle.
#![expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]

use std::marker::PhantomData;
use std::ops::{Add, Sub};

use occam_math::Real;

use crate::frame::{Displacement, Position, Role};

/// Two surface parameters. Use it through [`UvPoint`] and [`UvVector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair<M: Role, R: Real = f64> {
    u: R,
    v: R,
    meaning: PhantomData<M>,
}

/// A position in a surface's parameters.
pub type UvPoint<R = f64> = Pair<Position, R>;

/// A displacement in a surface's parameters.
pub type UvVector<R = f64> = Pair<Displacement, R>;

impl<M: Role, R: Real> Pair<M, R> {
    /// The parameters, in order.
    #[must_use]
    pub const fn new(u: R, v: R) -> Self {
        Self { u, v, meaning: PhantomData }
    }

    /// The first parameter.
    #[must_use]
    pub const fn u(self) -> R {
        self.u
    }

    /// The second parameter.
    #[must_use]
    pub const fn v(self) -> R {
        self.v
    }
}

impl<R: Real> Add<UvVector<R>> for UvPoint<R> {
    type Output = Self;

    fn add(self, rhs: UvVector<R>) -> Self {
        Self::new(self.u + rhs.u, self.v + rhs.v)
    }
}

impl<R: Real> Sub for UvPoint<R> {
    type Output = UvVector<R>;

    fn sub(self, rhs: Self) -> UvVector<R> {
        UvVector::new(self.u - rhs.u, self.v - rhs.v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surface_point_moves_by_a_displacement() {
        let moved = UvPoint::new(1.0, 2.0) + UvVector::new(10.0, 20.0);
        assert_eq!((moved.u(), moved.v()), (11.0, 22.0));
    }

    #[test]
    fn two_surface_points_differ_by_a_displacement() {
        let difference = UvPoint::new(11.0, 22.0) - UvPoint::new(1.0, 2.0);
        assert_eq!((difference.u(), difference.v()), (10.0, 20.0));
    }
}
