//! Source: original
//! The tolerance a model is built to (L22). It belongs to the model and is passed around with it: there is no default, and its
//! constructor is not `const`, so it cannot be a global.

use occam_report::{Code, NamedValue, Quantity, Report};

use crate::measure::{Angle, Length};

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
        Err(Report::new(Code::ToleranceOutOfRange).with_value(NamedValue::new("tolerance", quantity, value)))
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
    fn a_length_tolerance_out_of_range_is_refused_and_named() {
        for bad in [0.0, -1e-7, f64::NAN, f64::INFINITY] {
            let report = Tolerance::new(mm(bad), rad(1e-9)).unwrap_err();
            assert_eq!(report.code, Code::ToleranceOutOfRange, "{bad}");
            let named = report.value("tolerance").unwrap();
            assert_eq!((named.value.to_bits(), named.quantity), (bad.to_bits(), Quantity::Length));
        }
    }

    #[test]
    fn an_angle_tolerance_out_of_range_is_refused_and_named() {
        for bad in [0.0, -1e-9, f64::NAN, f64::INFINITY] {
            let report = Tolerance::new(mm(1e-7), rad(bad)).unwrap_err();
            assert_eq!(report.code, Code::ToleranceOutOfRange, "{bad}");
            let named = report.value("tolerance").unwrap();
            assert_eq!((named.value.to_bits(), named.quantity), (bad.to_bits(), Quantity::Angle));
        }
    }
}
