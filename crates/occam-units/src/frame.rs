//! Source: original
//! Points and vectors that carry their frame in their type, so that mixing frames does not compile (L22).

use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Neg, Sub};

use occam_math::Real;

use crate::measure::Length;

mod sealed {
    /// Only this crate says what a frame is, and what coordinates can mean.
    pub trait Sealed {}
    impl Sealed for super::World {}
    impl Sealed for super::Local {}
    impl Sealed for super::Position {}
    impl Sealed for super::Displacement {}
}

/// A three-dimensional coordinate frame.
pub trait Frame: sealed::Sealed + Copy + Debug + PartialEq + Send + Sync + 'static {}

/// The model's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World;

/// A frame local to one body or feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local;

impl Frame for World {}
impl Frame for Local {}

/// What a set of coordinates means.
pub trait Role: sealed::Sealed + Copy + Debug + PartialEq + Send + Sync + 'static {}

/// Coordinates that say where something is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position;

/// Coordinates that say how far something moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Displacement;

impl Role for Position {}
impl Role for Displacement {}

/// Three lengths in frame `F`. Use it through [`Point`] and [`Vector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triple<M: Role, F: Frame, R: Real = f64> {
    x: Length<R>,
    y: Length<R>,
    z: Length<R>,
    meaning: PhantomData<(M, F)>,
}

/// A position in frame `F`.
pub type Point<F, R = f64> = Triple<Position, F, R>;

/// A displacement in frame `F`.
pub type Vector<F, R = f64> = Triple<Displacement, F, R>;

impl<M: Role, F: Frame, R: Real> Triple<M, F, R> {
    /// The coordinates, in order.
    #[must_use]
    pub const fn new(x: Length<R>, y: Length<R>, z: Length<R>) -> Self {
        Self { x, y, z, meaning: PhantomData }
    }

    /// The x coordinate.
    #[must_use]
    pub const fn x(self) -> Length<R> {
        self.x
    }

    /// The y coordinate.
    #[must_use]
    pub const fn y(self) -> Length<R> {
        self.y
    }

    /// The z coordinate.
    #[must_use]
    pub const fn z(self) -> Length<R> {
        self.z
    }
}

impl<F: Frame, R: Real> Add<Vector<F, R>> for Point<F, R> {
    type Output = Self;

    fn add(self, rhs: Vector<F, R>) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl<F: Frame, R: Real> Sub<Vector<F, R>> for Point<F, R> {
    type Output = Self;

    fn sub(self, rhs: Vector<F, R>) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl<F: Frame, R: Real> Sub for Point<F, R> {
    type Output = Vector<F, R>;

    fn sub(self, rhs: Self) -> Vector<F, R> {
        Vector::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl<F: Frame, R: Real> Add for Vector<F, R> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl<F: Frame, R: Real> Sub for Vector<F, R> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl<F: Frame, R: Real> Neg for Vector<F, R> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl<F: Frame, R: Real> Mul<R> for Vector<F, R> {
    type Output = Self;

    fn mul(self, factor: R) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }
}

impl<F: Frame, R: Real> Div<R> for Vector<F, R> {
    type Output = Self;

    fn div(self, divisor: R) -> Self {
        Self::new(self.x / divisor, self.y / divisor, self.z / divisor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::new(mm(x), mm(y), mm(z))
    }

    fn vector(x: f64, y: f64, z: f64) -> Vector<World> {
        Vector::new(mm(x), mm(y), mm(z))
    }

    fn coords(p: Point<World>) -> [f64; 3] {
        [p.x(), p.y(), p.z()].map(Length::millimetres)
    }

    fn parts(v: Vector<World>) -> [f64; 3] {
        [v.x(), v.y(), v.z()].map(Length::millimetres)
    }

    #[test]
    fn a_point_moves_by_a_vector() {
        assert_eq!(coords(point(1.0, 2.0, 3.0) + vector(10.0, 20.0, 30.0)), [11.0, 22.0, 33.0]);
        assert_eq!(coords(point(11.0, 22.0, 33.0) - vector(10.0, 20.0, 30.0)), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn two_points_differ_by_a_vector() {
        assert_eq!(parts(point(11.0, 22.0, 33.0) - point(1.0, 2.0, 3.0)), [10.0, 20.0, 30.0]);
    }

    #[test]
    fn vectors_add_subtract_and_negate() {
        assert_eq!(parts(vector(1.0, 2.0, 3.0) + vector(10.0, 20.0, 30.0)), [11.0, 22.0, 33.0]);
        assert_eq!(parts(vector(11.0, 22.0, 33.0) - vector(10.0, 20.0, 30.0)), [1.0, 2.0, 3.0]);
        assert_eq!(parts(-vector(1.0, -2.0, 3.0)), [-1.0, 2.0, -3.0]);
    }

    #[test]
    fn a_vector_is_scaled_by_a_bare_number() {
        assert_eq!(parts(vector(1.0, 2.0, 3.0) * 2.0), [2.0, 4.0, 6.0]);
        assert_eq!(parts(vector(2.0, 4.0, 6.0) / 2.0), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn local_points_work_the_same_way() {
        let a: Point<Local> = Point::new(mm(1.0), mm(2.0), mm(3.0));
        let b: Vector<Local> = Vector::new(mm(1.0), mm(1.0), mm(1.0));
        assert_eq!((a + b).z().millimetres(), 4.0);
    }
}
