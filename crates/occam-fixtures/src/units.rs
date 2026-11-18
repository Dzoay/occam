//! Source: original
//! Quantities as the tests write them: millimetres and radians as bare numbers.

use occam_units::{Angle, Length, Point, Tolerance, Vec3, World};

/// `x` millimetres.
#[must_use]
pub const fn mm(x: f64) -> Length {
    Length::from_millimetres(x)
}

/// `x` radians.
#[must_use]
pub const fn rad(x: f64) -> Angle {
    Angle::from_radians(x)
}

/// The point at these coordinates, in millimetres.
#[must_use]
pub fn point(x: f64, y: f64, z: f64) -> Point<World> {
    Point::from_vec3(Vec3::new(x, y, z))
}

/// The tolerance a new model is built to by default (O12): a band of 1e-7 mm and an angle of 1e-9 rad.
#[must_use]
pub fn tol() -> Tolerance {
    tol_of(1e-7)
}

/// A tolerance with a band of `band` millimetres and the default angle of 1e-9 rad.
#[must_use]
pub fn tol_of(band: f64) -> Tolerance {
    Tolerance::new(mm(band), rad(1e-9)).unwrap()
}
