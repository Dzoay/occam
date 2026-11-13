//! Source: original
//! [`Placement`]: a rigid map from one frame to another, a rotation then a shift, built once with the owned `sin_cos`
//! (O14). Inputs arrive in `Local` and are placed into `World`, where every stored curve and surface lives.

use std::marker::PhantomData;

use occam_math::sin_cos;

use crate::direction::{Direction, Tangent};
use crate::frame::{Frame, Point, Vector};
use crate::measure::Angle;
use crate::vec3::Vec3;

/// A rigid map from frame `A` to frame `B`: `p_B = rotation p_A + shift`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement<A: Frame, B: Frame> {
    rows: [Vec3; 3],
    shift: Vec3,
    frames: PhantomData<(A, B)>,
}

impl<A: Frame, B: Frame> Placement<A, B> {
    /// The same coordinates in `B` as in `A`.
    #[must_use]
    pub const fn identity() -> Self {
        let rows = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        Self { rows, shift: Vec3::new(0.0, 0.0, 0.0), frames: PhantomData }
    }

    /// The rotation part applied to plain numbers.
    fn rotate(&self, v: Vec3) -> Vec3 {
        let [first, second, third] = self.rows;
        Vec3::new(first.dot(v), second.dot(v), third.dot(v))
    }

    /// A point of `A` in `B`.
    #[must_use]
    pub fn point(&self, p: Point<A>) -> Point<B> {
        Point::from_vec3(self.rotate(p.to_vec3()) + self.shift)
    }

    /// A vector of `A` in `B`: rotated, never shifted.
    #[must_use]
    pub fn vector(&self, v: Vector<A>) -> Vector<B> {
        Vector::from_vec3(self.rotate(v.to_vec3()))
    }

    /// A direction of `A` in `B`, made a unit again after the rotation's rounding.
    #[must_use]
    pub fn direction(&self, d: Direction<A>) -> Direction<B> {
        Direction::normalised(self.rotate(d.get()))
    }

    /// A tangent of `A` in `B`.
    #[must_use]
    pub fn tangent(&self, t: Tangent<A>) -> Tangent<B> {
        Tangent::new(self.rotate(t.rate()))
    }

    /// This map, then `next`.
    #[must_use]
    pub fn then<C: Frame>(&self, next: &Placement<B, C>) -> Placement<A, C> {
        let [first, second, third] = self.columns();
        let rows = next.rows.map(|row| Vec3::new(row.dot(first), row.dot(second), row.dot(third)));
        Placement { rows, shift: next.rotate(self.shift) + next.shift, frames: PhantomData }
    }

    /// The rotation's columns, which are the rows of its inverse.
    const fn columns(&self) -> [Vec3; 3] {
        let [a, b, c] = self.rows;
        [Vec3::new(a.x, b.x, c.x), Vec3::new(a.y, b.y, c.y), Vec3::new(a.z, b.z, c.z)]
    }

    /// The map back from `B` to `A`: the transposed rotation, and the shift undone.
    #[must_use]
    pub fn inverse(&self) -> Placement<B, A> {
        let back = Placement { rows: self.columns(), shift: Vec3::new(0.0, 0.0, 0.0), frames: PhantomData };
        Placement { shift: -back.rotate(self.shift), ..back }
    }
}

impl<F: Frame> Placement<F, F> {
    /// A turn by `angle` about `axis` through the origin, right-handed (Rodrigues' formula).
    #[must_use]
    pub fn rotation(axis: Direction<F>, angle: Angle) -> Self {
        let (sin, cos) = sin_cos(angle.radians());
        let k = axis.get();
        let turn = 1.0 - cos;
        let rows = [
            Vec3::new(turn * k.x * k.x + cos, turn * k.x * k.y - sin * k.z, turn * k.x * k.z + sin * k.y),
            Vec3::new(turn * k.x * k.y + sin * k.z, turn * k.y * k.y + cos, turn * k.y * k.z - sin * k.x),
            Vec3::new(turn * k.x * k.z - sin * k.y, turn * k.y * k.z + sin * k.x, turn * k.z * k.z + cos),
        ];
        Self { rows, shift: Vec3::new(0.0, 0.0, 0.0), frames: PhantomData }
    }

    /// A shift by `by`.
    #[must_use]
    pub fn translation(by: Vector<F>) -> Self {
        Self { shift: by.to_vec3(), ..Self::identity() }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::frame::{Local, World};
    use crate::measure::Length;
    use crate::tolerance::Tolerance;

    fn point<F: Frame>(x: f64, y: f64, z: f64) -> Point<F> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    fn distance<F: Frame>(a: Point<F>, b: Point<F>) -> f64 {
        (a - b).to_vec3().norm()
    }

    fn band() -> f64 {
        let tolerance = Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).unwrap();
        tolerance.band().millimetres()
    }

    fn z_axis<F: Frame>() -> Direction<F> {
        Direction::normalised(Vec3::new(0.0, 0.0, 1.0))
    }

    #[test]
    fn a_quarter_turn_about_z_takes_x_to_y() {
        let quarter = Placement::<World, World>::rotation(z_axis(), Angle::from_radians(FRAC_PI_2));
        let moved = quarter.point(point(1.0, 0.0, 0.0));
        assert!(distance(moved, point(0.0, 1.0, 0.0)) < 1e-15, "{moved:?}");
        let turned = quarter.direction(Direction::normalised(Vec3::new(0.0, 1.0, 0.0)));
        assert!((turned.get() - Vec3::new(-1.0, 0.0, 0.0)).norm() < 1e-15);
    }

    #[test]
    fn a_shift_moves_points_but_not_vectors() {
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(shift.point(point(1.0, 1.0, 1.0)), point(2.0, 3.0, 4.0));
        assert_eq!(shift.vector(Vector::from_vec3(Vec3::new(1.0, 1.0, 1.0))).to_vec3(), Vec3::new(1.0, 1.0, 1.0));
        let tangent = shift.tangent(Tangent::new(Vec3::new(0.0, 5.0, 0.0)));
        assert_eq!(tangent.rate(), Vec3::new(0.0, 5.0, 0.0));
    }

    #[test]
    fn composing_applies_the_first_then_the_second() {
        let turn = Placement::<World, World>::rotation(z_axis(), Angle::from_radians(FRAC_PI_2));
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(10.0, 0.0, 0.0)));
        let both = turn.then(&shift);
        assert!(distance(both.point(point(1.0, 0.0, 0.0)), point(10.0, 1.0, 0.0)) < 1e-14);
        let other_way = shift.then(&turn);
        assert!(distance(other_way.point(point(1.0, 0.0, 0.0)), point(0.0, 11.0, 0.0)) < 1e-14);
    }

    #[test]
    fn a_placement_and_its_inverse_round_trip_within_the_band() {
        let axis = Direction::<Local>::normalised(Vec3::new(1.0, -2.0, 0.5));
        let tilt = Placement::<Local, Local>::rotation(axis, Angle::from_radians(0.7));
        let place = tilt
            .then(&Placement::<Local, Local>::translation(Vector::from_vec3(Vec3::new(120.0, -40.0, 7.5))))
            .then(&Placement::<Local, World>::identity());
        let back = place.inverse();
        for p in [point::<Local>(0.0, 0.0, 0.0), point(250.0, -13.0, 99.5), point(-0.001, 1e-6, 3000.0)] {
            let there: Point<World> = place.point(p);
            assert!(distance(back.point(there), p) <= band(), "{p:?}");
        }
    }

    #[test]
    fn the_identity_changes_nothing() {
        let same = Placement::<Local, World>::identity();
        assert_eq!(same.point(point(1.5, -2.5, 3.5)), point(1.5, -2.5, 3.5));
        assert_eq!(same.inverse().point(point(1.5, -2.5, 3.5)), point::<Local>(1.5, -2.5, 3.5));
    }
}
