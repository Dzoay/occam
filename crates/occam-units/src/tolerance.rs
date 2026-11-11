//! Source: original
//! The tolerance a model is built to (L22). It belongs to the model and is passed around with it: there is no default, and its
//! constructor is not `const`, so it cannot be a global.

use occam_report::{Code, Fault, NamedValue, Quantity, Report};

use crate::measure::{Angle, Length};

/// What a curve or surface parameter measures, so that a test in parameter space uses the right band. S4's cone compared
/// a length parameter with an angle tolerance. Parameters stay bare numbers, so lane kernels read plain `f64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// A distance, in millimetres.
    Length,
    /// A turn, in radians, which comes round again.
    Angle,
    /// A B-spline's knot value, with no unit of its own.
    Knot,
}

/// How close two lengths or angles must be to count as the same in one model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    length: Length,
    angle: Angle,
}

/// Refuses anything but a finite number greater than zero, which NaN is not.
fn in_range(value: f64, quantity: Quantity) -> Result<(), Report> {
    if value > 0.0 && value.is_finite() {
        Ok(())
    } else {
        Err(Report::new(Code::ToleranceOutOfRange, Fault::InvalidInput).with_value(NamedValue::new(
            "tolerance",
            quantity,
            value,
        )))
    }
}

impl Tolerance {
    /// A model's tolerance.
    ///
    /// # Errors
    ///
    /// [`Code::ToleranceOutOfRange`] if either is not a finite number greater than zero.
    pub fn new(length: Length, angle: Angle) -> Result<Self, Report> {
        in_range(length.millimetres(), Quantity::Length)?;
        in_range(angle.radians(), Quantity::Angle)?;
        Ok(Self { length, angle })
    }

    /// How close two lengths must be.
    #[must_use]
    pub const fn length(self) -> Length {
        self.length
    }

    /// How close two angles must be.
    #[must_use]
    pub const fn angle(self) -> Angle {
        self.angle
    }

    /// What every decision compares with: `|q| <= band` is on, tangent or the same. It never grows (S2).
    #[must_use]
    pub const fn band(self) -> Length {
        self.length
    }

    /// How far a band decision at near tangency moves a point on a curvature radius `radius`: `sqrt(2 radius band)`
    /// (S2 item 4). It goes into a report as a named value, never into a vertex's certified radius.
    #[must_use]
    pub fn near_tangent_shift(self, radius: Length) -> Length {
        Length::from_millimetres((2.0 * radius.millimetres().abs() * self.length.millimetres()).sqrt())
    }

    /// The band in a parameter of `kind` where a unit of the parameter moves the point by `rate`: the parameter change
    /// that moves it by the length band. A turn's band is at most π, beyond which it comes round again; where `rate` is
    /// zero, as at a pole, every value of a length or knot parameter is the same point.
    #[must_use]
    pub fn in_parameter(self, kind: ParamKind, rate: Length) -> f64 {
        let band = self.length.millimetres() / rate.millimetres().abs();
        match kind {
            ParamKind::Angle if band.partial_cmp(&std::f64::consts::PI) != Some(std::cmp::Ordering::Less) => {
                std::f64::consts::PI
            }
            ParamKind::Length | ParamKind::Angle | ParamKind::Knot => band,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn rad(x: f64) -> Angle {
        Angle::from_radians(x)
    }

    #[test]
    fn a_positive_tolerance_keeps_what_it_was_given() {
        let tolerance = Tolerance::new(mm(1e-7), rad(1e-9)).unwrap();
        assert_eq!(tolerance.length(), mm(1e-7));
        assert_eq!(tolerance.angle(), rad(1e-9));
    }

    #[test]
    fn the_band_is_the_length_tolerance() {
        assert_eq!(Tolerance::new(mm(1e-7), rad(1e-9)).unwrap().band(), mm(1e-7));
    }

    #[test]
    fn the_near_tangent_shift_is_the_square_root_of_twice_the_radius_and_the_band() {
        let tolerance = Tolerance::new(mm(2e-6), rad(1e-9)).unwrap();
        assert_eq!(tolerance.near_tangent_shift(mm(25.0)), mm(0.01));
        assert_eq!(tolerance.near_tangent_shift(mm(-25.0)), mm(0.01));
    }

    #[test]
    fn the_band_in_a_parameter_is_the_change_that_moves_the_point_by_the_band() {
        let tolerance = Tolerance::new(mm(1e-6), rad(1e-9)).unwrap();
        assert_eq!(tolerance.in_parameter(ParamKind::Length, mm(2.0)), 5e-7);
        assert_eq!(tolerance.in_parameter(ParamKind::Knot, mm(-4.0)), 2.5e-7);
        // A cone's apex angle on a radius of 10 mm: the length band, not the angle tolerance (S4).
        assert_eq!(tolerance.in_parameter(ParamKind::Angle, mm(10.0)), 1e-7);
        assert_eq!(tolerance.in_parameter(ParamKind::Angle, mm(0.0)), std::f64::consts::PI, "at a pole every angle");
        assert_eq!(tolerance.in_parameter(ParamKind::Angle, mm(1e-9)), std::f64::consts::PI);
        assert_eq!(tolerance.in_parameter(ParamKind::Length, mm(0.0)), f64::INFINITY);
    }

    #[test]
    fn a_length_tolerance_out_of_range_is_refused_and_named() {
        for bad in [0.0, -1e-7, f64::NAN, f64::INFINITY] {
            let report = Tolerance::new(mm(bad), rad(1e-9)).unwrap_err();
            assert_eq!(report.code, Code::ToleranceOutOfRange, "{bad}");
            let named = report.value("tolerance").unwrap();
            assert_eq!((named.number().unwrap().to_bits(), named.quantity), (bad.to_bits(), Quantity::Length));
        }
    }

    #[test]
    fn an_angle_tolerance_out_of_range_is_refused_and_named() {
        for bad in [0.0, -1e-9, f64::NAN, f64::INFINITY] {
            let report = Tolerance::new(mm(1e-7), rad(bad)).unwrap_err();
            assert_eq!(report.code, Code::ToleranceOutOfRange, "{bad}");
            let named = report.value("tolerance").unwrap();
            assert_eq!((named.number().unwrap().to_bits(), named.quantity), (bad.to_bits(), Quantity::Angle));
        }
    }
}
