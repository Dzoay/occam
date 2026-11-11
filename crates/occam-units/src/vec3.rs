//! Source: original
//! [`Vec3`]: three plain numbers for the inside of a formula, typed again at its ends. S1 found the typed layer stopped
//! where formulas began, for want of dot, cross and norm.

use std::ops::{Add, Mul, Neg, Sub};

use occam_math::Real;

use crate::frame::{Frame, Role, Triple};
use crate::measure::Length;

/// Three numbers side by side, with the vector algebra a formula needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3<R: Real = f64> {
    /// The first.
    pub x: R,
    /// The second.
    pub y: R,
    /// The third.
    pub z: R,
}

impl<R: Real> Vec3<R> {
    /// The three numbers.
    #[inline(always)]
    pub const fn new(x: R, y: R, z: R) -> Self {
        Self { x, y, z }
    }

    /// The dot product.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub fn dot(self, other: Self) -> R {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// The cross product, right-handed.
    #[must_use]
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// The length, as the square root of the dot product with itself.
    #[inline(always)]
    pub fn norm(self) -> R {
        self.dot(self).sqrt()
    }
}

impl<R: Real> Add for Vec3<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl<R: Real> Sub for Vec3<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl<R: Real> Neg for Vec3<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl<R: Real> Mul<R> for Vec3<R> {
    type Output = Self;

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn mul(self, factor: R) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }
}

impl<M: Role, F: Frame, R: Real> Triple<M, F, R> {
    /// The coordinates in millimetres, as plain numbers for a formula.
    #[inline(always)]
    pub const fn to_vec3(self) -> Vec3<R> {
        Vec3::new(self.x().millimetres(), self.y().millimetres(), self.z().millimetres())
    }

    /// Coordinates in millimetres from a formula, typed again.
    #[inline(always)]
    pub const fn from_vec3(v: Vec3<R>) -> Self {
        Self::new(Length::from_millimetres(v.x), Length::from_millimetres(v.y), Length::from_millimetres(v.z))
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Lanes;

    use super::*;
    use crate::frame::{Point, World};

    #[test]
    fn dot_cross_and_norm_follow_the_right_hand_rule() {
        let (x, y) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(y.cross(x), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(Vec3::new(1.0, 2.0, 3.0).dot(Vec3::new(4.0, -5.0, 6.0)), 12.0);
        assert_eq!(Vec3::new(3.0, 4.0, 12.0).norm(), 13.0);
    }

    #[test]
    fn the_operators_work_component_by_component() {
        let (a, b) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.5, -1.0, 4.0));
        assert_eq!(a + b, Vec3::new(1.5, 1.0, 7.0));
        assert_eq!(a - b, Vec3::new(0.5, 3.0, -1.0));
        assert_eq!(-a, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn a_point_becomes_numbers_and_back() {
        let p: Point<World> = Point::from_vec3(Vec3::new(1.5, -2.0, 3.25));
        assert_eq!(p.x().millimetres(), 1.5);
        assert_eq!(p.to_vec3(), Vec3::new(1.5, -2.0, 3.25));
    }

    #[test]
    fn it_runs_in_lanes() {
        let a = Vec3::new(Lanes::new([1.0, 0.0]), Lanes::new([0.0, 1.0]), Lanes::new([0.0, 0.0]));
        let b = Vec3::new(Lanes::new([0.0, 1.0]), Lanes::new([1.0, 0.0]), Lanes::new([0.0, 0.0]));
        assert_eq!(a.cross(b).z.to_array(), [1.0, -1.0]);
        assert_eq!(a.norm().to_array(), [1.0, 1.0]);
    }
}
