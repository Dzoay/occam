//! Source: original
//! [`Direction`] and [`Tangent`]: a unit axis is not a vector, which is three lengths, and a derivative is a length per
//! unit of a parameter that may be a length, an angle or a knot unit (S4). Neither can be added to a point.

use std::marker::PhantomData;

use occam_math::Real;
use occam_report::{Code, NamedValue, Report};

use crate::frame::{Frame, Vector};
use crate::tolerance::Tolerance;
use crate::vec3::Vec3;

/// A dimensionless unit axis in frame `F`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction<F: Frame, R: Real = f64> {
    unit: Vec3<R>,
    frame: PhantomData<F>,
}

impl<F: Frame> Direction<F> {
    /// The direction of `v`.
    ///
    /// # Errors
    ///
    /// [`Code::DegenerateDirection`] if `v` is no longer than the model's length tolerance, or is not finite.
    pub fn along(v: Vector<F>, tolerance: Tolerance) -> Result<Self, Report> {
        let length = v.to_vec3().norm();
        let band = tolerance.band().millimetres();
        if length > band && length.is_finite() {
            Ok(Self::normalised(v.to_vec3()))
        } else {
            Err(Report::new(Code::DegenerateDirection)
                .with_value(NamedValue::length("length", length))
                .with_value(NamedValue::length("tolerance", band)))
        }
    }
}

impl<F: Frame, R: Real> Direction<F, R> {
    /// `v` divided by its length, for a formula that has just computed a unit axis, as a normal is. NaN in every lane
    /// where `v` is zero or not finite: an undefined direction is NaN, never a guess.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub fn normalised(v: Vec3<R>) -> Self {
        let length = v.norm();
        Self { unit: Vec3::new(v.x / length, v.y / length, v.z / length), frame: PhantomData }
    }

    /// The unit axis, as plain numbers.
    #[inline(always)]
    pub const fn get(self) -> Vec3<R> {
        self.unit
    }

    /// The opposite direction.
    #[must_use]
    #[inline(always)]
    pub fn reversed(self) -> Self {
        Self { unit: -self.unit, frame: PhantomData }
    }
}

/// A derivative in frame `F`: length per unit of a parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tangent<F: Frame, R: Real = f64> {
    rate: Vec3<R>,
    frame: PhantomData<F>,
}

impl<F: Frame, R: Real> Tangent<F, R> {
    /// The rate of change of a position with a parameter, in millimetres per unit of that parameter.
    #[inline(always)]
    pub const fn new(rate: Vec3<R>) -> Self {
        Self { rate, frame: PhantomData }
    }

    /// The rate, as plain numbers.
    #[inline(always)]
    pub const fn rate(self) -> Vec3<R> {
        self.rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::World;
    use crate::measure::{Angle, Length};

    fn tolerance() -> Tolerance {
        Tolerance::new(Length::from_millimetres(1e-6), Angle::from_radians(1e-9)).unwrap()
    }

    fn vector(x: f64, y: f64, z: f64) -> Vector<World> {
        Vector::from_vec3(Vec3::new(x, y, z))
    }

    #[test]
    fn a_direction_is_a_unit_axis() {
        let d = Direction::along(vector(0.0, 3.0, 4.0), tolerance()).unwrap();
        assert_eq!(d.get(), Vec3::new(0.0, 0.6, 0.8));
        assert_eq!(d.reversed().get(), Vec3::new(-0.0, -0.6, -0.8));
    }

    #[test]
    fn a_vector_within_the_tolerance_has_no_direction() {
        let report = Direction::along(vector(1e-7, 0.0, 0.0), tolerance()).unwrap_err();
        assert_eq!(report.code, Code::DegenerateDirection);
        assert_eq!(report.value("length").map(|v| v.value), Some(1e-7));
        assert_eq!(report.value("tolerance").map(|v| v.value), Some(1e-6));
        assert!(Direction::along(vector(1e-6, 0.0, 0.0), tolerance()).is_err(), "the band includes its edge");
        assert!(Direction::along(vector(2e-6, 0.0, 0.0), tolerance()).is_ok());
        assert!(Direction::along(vector(f64::INFINITY, 0.0, 0.0), tolerance()).is_err());
        assert!(Direction::along(vector(f64::NAN, 1.0, 0.0), tolerance()).is_err());
    }

    #[test]
    fn an_undefined_normalised_direction_is_nan() {
        let d: Direction<World> = Direction::normalised(Vec3::new(0.0, 0.0, 0.0));
        assert!(d.get().x.is_nan() && d.get().y.is_nan() && d.get().z.is_nan());
    }

    #[test]
    fn a_tangent_keeps_its_rate() {
        let t: Tangent<World> = Tangent::new(Vec3::new(2.0, 0.0, -1.0));
        assert_eq!(t.rate(), Vec3::new(2.0, 0.0, -1.0));
    }

    #[test]
    fn the_degenerate_direction_code_has_no_remedy_to_follow() {
        assert_eq!(Code::DegenerateDirection.entry().remedies, &[] as &[occam_report::RemedyKind]);
    }
}
