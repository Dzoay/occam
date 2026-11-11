//! Source: original
//! [`Length`] and [`Angle`]: numbers that say what they measure. Each is held in occam's internal unit.
#![expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]

use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Neg, Sub};

use occam_math::Real;
use occam_report::{AngleUnit, Code, Fault, LengthUnit, NamedValue, Quantity, Report};

mod sealed {
    /// Only this crate says what can be measured.
    pub trait Sealed {}
    impl Sealed for super::Millimetres {}
    impl Sealed for super::Radians {}
}

/// An internal unit a [`Measure`] is held in.
pub trait Unit: sealed::Sealed + Copy + Debug + PartialEq + Send + Sync + 'static {}

/// The internal unit of length.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Millimetres;

/// The internal unit of angle.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Radians;

impl Unit for Millimetres {}
impl Unit for Radians {}

/// A number in the internal unit `U`. Use it through [`Length`] and [`Angle`].
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Measure<U: Unit, R: Real = f64> {
    value: R,
    unit: PhantomData<U>,
}

/// A length, held in millimetres.
pub type Length<R = f64> = Measure<Millimetres, R>;

/// An angle, held in radians.
pub type Angle<R = f64> = Measure<Radians, R>;

/// `internal` if it is finite. Otherwise the report names `given`, the number as the caller gave it.
fn checked(internal: f64, given: f64) -> Result<f64, Report> {
    if internal.is_finite() {
        Ok(internal)
    } else {
        Err(Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::new(
            "value",
            Quantity::Ratio,
            given,
        )))
    }
}

impl Length<f64> {
    /// The length of `value` in `unit`: the one place a number from outside becomes a length.
    ///
    /// # Errors
    ///
    /// [`Code::NotFinite`] if `value` is NaN or infinite, or is too large to hold in millimetres.
    pub fn new(value: f64, unit: LengthUnit) -> Result<Self, Report> {
        Ok(Self::from_millimetres(checked(value * unit.millimetres(), value)?))
    }

    /// This length in `unit`.
    #[must_use]
    pub fn value_in(self, unit: LengthUnit) -> f64 {
        self.millimetres() / unit.millimetres()
    }
}

impl Angle<f64> {
    /// The angle of `value` in `unit`: the one place a number from outside becomes an angle.
    ///
    /// # Errors
    ///
    /// [`Code::NotFinite`] if `value` is NaN or infinite.
    pub fn new(value: f64, unit: AngleUnit) -> Result<Self, Report> {
        Ok(Self::from_radians(checked(value / unit.per_radian(), value)?))
    }

    /// This angle in `unit`.
    #[must_use]
    pub fn value_in(self, unit: AngleUnit) -> f64 {
        self.radians() * unit.per_radian()
    }
}

impl<R: Real> Length<R> {
    /// A length from a number already in millimetres, for the kernel's own results. Numbers from outside use [`Length::new`].
    #[must_use]
    pub const fn from_millimetres(millimetres: R) -> Self {
        Self { value: millimetres, unit: PhantomData }
    }

    /// This length in millimetres.
    #[must_use]
    pub const fn millimetres(self) -> R {
        self.value
    }
}

impl<R: Real> Angle<R> {
    /// An angle from a number already in radians, for the kernel's own results. Numbers from outside use [`Angle::new`].
    #[must_use]
    pub const fn from_radians(radians: R) -> Self {
        Self { value: radians, unit: PhantomData }
    }

    /// This angle in radians.
    #[must_use]
    pub const fn radians(self) -> R {
        self.value
    }
}

impl<U: Unit, R: Real> Measure<U, R> {
    const fn with(value: R) -> Self {
        Self { value, unit: PhantomData }
    }

    /// The size, without its sign.
    #[must_use]
    pub fn abs(self) -> Self {
        Self::with(self.value.abs())
    }
}

impl<U: Unit, R: Real> Add for Measure<U, R> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::with(self.value + rhs.value)
    }
}

impl<U: Unit, R: Real> Sub for Measure<U, R> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::with(self.value - rhs.value)
    }
}

impl<U: Unit, R: Real> Neg for Measure<U, R> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::with(-self.value)
    }
}

impl<U: Unit, R: Real> Mul<R> for Measure<U, R> {
    type Output = Self;

    fn mul(self, factor: R) -> Self {
        Self::with(self.value * factor)
    }
}

impl<U: Unit, R: Real> Div<R> for Measure<U, R> {
    type Output = Self;

    fn div(self, divisor: R) -> Self {
        Self::with(self.value / divisor)
    }
}

impl<U: Unit, R: Real> Div for Measure<U, R> {
    type Output = R;

    fn div(self, divisor: Self) -> R {
        self.value / divisor.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTH_UNITS: [LengthUnit; 4] =
        [LengthUnit::Millimetre, LengthUnit::Centimetre, LengthUnit::Metre, LengthUnit::Inch];
    const ANGLE_UNITS: [AngleUnit; 2] = [AngleUnit::Radian, AngleUnit::Degree];
    const NUMBERS: [f64; 9] = [1.0, 0.1, 25.4, 1e-7, 1e9, 1.0 / 3.0, -2.5, 0.0, 180.0];

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn rad(x: f64) -> Angle {
        Angle::from_radians(x)
    }

    /// Multiplying and dividing by the same factor rounds twice, so a round trip is within two parts in 2^52.
    fn close(got: f64, want: f64) -> bool {
        (got - want).abs() <= 2.0 * f64::EPSILON * want.abs()
    }

    #[test]
    fn measures_compare_by_size() {
        assert!(Length::from_millimetres(1.0) < Length::from_millimetres(2.0));
        assert!(Angle::from_radians(2.0) > Angle::from_radians(1.0));
        assert_eq!(Length::from_millimetres(f64::NAN).partial_cmp(&Length::from_millimetres(1.0)), None);
    }

    #[test]
    fn a_length_is_converted_to_millimetres_on_the_way_in() {
        assert_eq!(Length::new(1.0, LengthUnit::Millimetre).unwrap().millimetres(), 1.0);
        assert_eq!(Length::new(1.0, LengthUnit::Centimetre).unwrap().millimetres(), 10.0);
        assert_eq!(Length::new(1.0, LengthUnit::Metre).unwrap().millimetres(), 1000.0);
        assert_eq!(Length::new(1.0, LengthUnit::Inch).unwrap().millimetres(), 25.4);
    }

    #[test]
    fn an_angle_is_converted_to_radians_on_the_way_in() {
        assert_eq!(Angle::new(2.5, AngleUnit::Radian).unwrap().radians(), 2.5);
        assert!(close(Angle::new(180.0, AngleUnit::Degree).unwrap().radians(), std::f64::consts::PI));
    }

    #[test]
    fn a_length_is_shown_in_the_units_it_is_asked_for() {
        assert_eq!(mm(25.4).value_in(LengthUnit::Inch), 1.0);
        assert_eq!(mm(1000.0).value_in(LengthUnit::Metre), 1.0);
        assert_eq!(mm(10.0).value_in(LengthUnit::Centimetre), 1.0);
        assert_eq!(mm(7.0).value_in(LengthUnit::Millimetre), 7.0);
    }

    #[test]
    fn an_angle_is_shown_in_the_units_it_is_asked_for() {
        assert!(close(rad(std::f64::consts::PI).value_in(AngleUnit::Degree), 180.0));
        assert_eq!(rad(2.5).value_in(AngleUnit::Radian), 2.5);
    }

    #[test]
    fn a_length_survives_the_round_trip_through_every_unit() {
        for unit in LENGTH_UNITS {
            for x in NUMBERS {
                assert!(close(Length::new(x, unit).unwrap().value_in(unit), x), "{x} {unit:?}");
            }
        }
    }

    #[test]
    fn an_angle_survives_the_round_trip_through_every_unit() {
        for unit in ANGLE_UNITS {
            for x in NUMBERS {
                assert!(close(Angle::new(x, unit).unwrap().value_in(unit), x), "{x} {unit:?}");
            }
        }
    }

    #[test]
    fn a_number_that_is_not_finite_is_refused_and_named() {
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let report = Length::new(x, LengthUnit::Millimetre).unwrap_err();
            assert_eq!(report.code, Code::NotFinite);
            assert_eq!(report.value("value").unwrap().number().unwrap().to_bits(), x.to_bits());
            assert_eq!(Angle::new(x, AngleUnit::Radian).unwrap_err().code, Code::NotFinite);
        }
    }

    #[test]
    fn a_finite_number_too_large_for_millimetres_is_refused_as_the_caller_gave_it() {
        let report = Length::new(1e308, LengthUnit::Metre).unwrap_err();
        assert_eq!(report.code, Code::NotFinite);
        assert_eq!(report.value("value").unwrap().number(), Some(1e308));
    }

    #[test]
    fn a_number_is_reported_as_a_plain_ratio() {
        let report = Length::new(f64::NAN, LengthUnit::Inch).unwrap_err();
        assert_eq!(report.value("value").unwrap().quantity, Quantity::Ratio);
    }

    #[test]
    fn measures_add_subtract_and_negate_in_their_own_kind() {
        assert_eq!((mm(1.5) + mm(2.0)).millimetres(), 3.5);
        assert_eq!((mm(1.5) - mm(2.0)).millimetres(), -0.5);
        assert_eq!((-mm(1.5)).millimetres(), -1.5);
        assert_eq!((rad(1.5) + rad(2.0)).radians(), 3.5);
    }

    #[test]
    fn a_measure_is_scaled_by_a_bare_number() {
        assert_eq!((mm(1.5) * 2.0).millimetres(), 3.0);
        assert_eq!((mm(3.0) / 2.0).millimetres(), 1.5);
    }

    #[test]
    fn two_measures_of_a_kind_give_a_bare_ratio() {
        assert_eq!(mm(3.0) / mm(2.0), 1.5);
    }

    #[test]
    fn the_absolute_value_drops_the_sign() {
        assert_eq!(mm(-1.5).abs().millimetres(), 1.5);
        assert_eq!(mm(1.5).abs().millimetres(), 1.5);
    }

    #[test]
    fn measures_work_in_lanes() {
        use occam_math::Lanes;
        let a = Length::from_millimetres(Lanes::new([1.0, 2.0]));
        let b = Length::from_millimetres(Lanes::new([3.0, 4.0]));
        assert_eq!((a + b).millimetres().to_array(), [4.0, 6.0]);
    }
}
