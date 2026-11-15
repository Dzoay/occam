//! Source: original
//! The limits a mesh is made to (P9): its chordal deflection and its angle, the floor no surface may push an edge's step
//! below, and the most samples one edge may have. They are read before any geometry, so that no estimate gone wrong can
//! ask for more than they allow.

use occam_math::cos;
use occam_report::{Code, Fault, Input, NamedValue, Quantity, Report};
use occam_units::{Angle, Length};

/// What a mesh is made to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    deflection: Length,
    angle: Angle,
}

/// `value` as a limit's, refused unless it is a finite number greater than zero, which NaN is not.
fn positive(parameter: &str, quantity: Quantity, value: f64) -> Result<(), Report> {
    if value > 0.0 && value.is_finite() {
        return Ok(());
    }
    let report = Report::new(Code::LimitNotPositive, Fault::InvalidInput).with_input(Input::at(parameter));
    Err(report.with_value(NamedValue::new("value", quantity, value)))
}

/// The lower of two numbers: `a` where they tie or `b` is not a number, without `f64::min`'s choice between zeros.
pub(crate) fn lower(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The higher of two numbers: `a` where they tie or `b` is not a number.
pub(crate) fn higher(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

impl Limits {
    /// The most samples one edge may have, its ends included: P9's cap, `2^14`.
    pub const SAMPLES_PER_EDGE: u32 = 1 << 14;

    /// The limits of a mesh no point of whose edges' chords is further than `deflection` from its edge, and none of
    /// whose edges turns through more than `angle` between two samples. An angle of a whole turn or more leaves the
    /// turning to the deflection.
    ///
    /// # Errors
    ///
    /// [`Code::LimitNotPositive`] for a deflection or an angle that is not a finite number greater than zero.
    pub fn new(deflection: Length, angle: Angle) -> Result<Self, Report> {
        positive("deflection", Quantity::Length, deflection.millimetres())?;
        positive("angle", Quantity::Angle, angle.radians())?;
        Ok(Self { deflection, angle })
    }

    /// The farthest a chord may be from what it stands for.
    #[must_use]
    pub const fn deflection(self) -> Length {
        self.deflection
    }

    /// The most an edge may turn between two samples.
    #[must_use]
    pub const fn angle(self) -> Angle {
        self.angle
    }

    /// The shortest step any surface may ask of an edge: the deflection itself (P9: "never ask for a size below the
    /// deflection"). A piece of an edge no longer than twice the floor is never cut, so one surface's wrong estimate
    /// costs at most the edge's length over the floor, and a coarse enough deflection always fits under the cap.
    #[must_use]
    pub const fn floor(self) -> Length {
        self.deflection
    }

    /// The longest step a surface bending by `curvature` per millimetre asks of the edges of its faces: a chord whose
    /// height over an arc of that curvature is half the deflection, leaving its faces the other half, and no longer
    /// than turns through the angle; never below the floor. A flat surface asks for no limit at all.
    pub(crate) fn step(self, curvature: f64) -> f64 {
        let deflection = self.deflection.millimetres();
        let chordal = 2.0 * (deflection / curvature).sqrt();
        let angular = self.angle.radians() / curvature;
        higher(deflection, lower(chordal, angular))
    }

    /// The cosine of half the angle: what the angle between two tangents half a piece apart is held to.
    pub(crate) fn half_turn_cosine(self) -> f64 {
        cos(0.5 * self.angle.radians())
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::PI;

    use super::*;

    fn limits(deflection: f64, angle: f64) -> Result<Limits, Report> {
        Limits::new(Length::from_millimetres(deflection), Angle::from_radians(angle))
    }

    /// The parameter and the value a refusal names.
    fn refused(report: &Report) -> (Code, String, Option<f64>) {
        let parameter = report.input.as_ref().map(|input| input.parameter.clone()).unwrap_or_default();
        (report.code, parameter, report.value("value").and_then(NamedValue::number))
    }

    #[test]
    fn limits_keep_what_they_are_given_and_the_floor_is_the_deflection() {
        let given = limits(0.25, 0.5).unwrap();
        assert_eq!(given.deflection().millimetres(), 0.25);
        assert_eq!(given.angle().radians(), 0.5);
        assert_eq!(given.floor().millimetres(), 0.25);
    }

    #[test]
    fn a_limit_that_is_not_a_finite_number_above_zero_is_refused_by_name() {
        for bad in [0.0, -1.0, f64::INFINITY] {
            assert_eq!(
                refused(&limits(bad, 0.5).unwrap_err()),
                (Code::LimitNotPositive, "deflection".into(), Some(bad))
            );
            assert_eq!(refused(&limits(0.25, bad).unwrap_err()), (Code::LimitNotPositive, "angle".into(), Some(bad)));
        }
        let (code, parameter, value) = refused(&limits(f64::NAN, 0.5).unwrap_err());
        assert_eq!((code, parameter.as_str()), (Code::LimitNotPositive, "deflection"));
        assert!(value.is_some_and(f64::is_nan));
        let shown = limits(0.0, 0.5).unwrap_err().message(occam_report::DisplayUnits::default());
        assert_eq!(shown, "deflection is 0 mm, which is not a finite number greater than zero.");
    }

    #[test]
    fn a_surface_asks_for_the_chord_half_the_deflection_high_or_the_angle_whichever_is_shorter() {
        // A radius of 2 at a deflection of 0.01: the chord of height 0.005 is 2 sqrt(0.02) long; an angle of 1 asks for
        // 2, and one of 0.1 for 0.2, shorter than the chord.
        let chordal = 2.0 * 0.02_f64.sqrt();
        assert_eq!(limits(0.01, 1.0).unwrap().step(0.5), chordal);
        assert_eq!(limits(0.01, 0.1).unwrap().step(0.5), 0.2);
        assert_eq!(limits(0.01, PI).unwrap().step(0.0), f64::INFINITY, "a flat surface asks for no limit");
    }

    #[test]
    fn no_surface_asks_for_a_step_below_the_floor() {
        // Curvature near 1e15, as P9's collapsed row read it, would ask for a step of about 6e-9.
        assert_eq!(limits(0.001, 0.35).unwrap().step(1e15), 0.001);
        assert_eq!(limits(0.001, 0.35).unwrap().step(f64::INFINITY), 0.001);
        assert_eq!(limits(0.001, 0.35).unwrap().step(f64::NAN), 0.001, "no estimate asks for the floor");
    }

    #[test]
    fn the_angle_is_held_at_half_a_piece() {
        assert_eq!(limits(0.01, 1.0).unwrap().half_turn_cosine(), cos(0.5));
    }
}
