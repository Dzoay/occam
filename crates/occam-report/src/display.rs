//! Source: original
//! The units numbers are shown in, which are the caller's (L9). This is only for wording: values in a report stay in occam's
//! internal units, millimetres and radians.

text_enum! {
    /// A unit to show lengths in.
    LengthUnit {
        /// Millimetres.
        Millimetre = "mm",
        /// Centimetres.
        Centimetre = "cm",
        /// Metres.
        Metre = "m",
        /// Inches.
        Inch = "in",
    }
}

text_enum! {
    /// A unit to show angles in.
    AngleUnit {
        /// Radians.
        Radian = "rad",
        /// Degrees.
        Degree = "deg",
    }
}

impl LengthUnit {
    /// How many millimetres make one of this unit. Dividing by this is exact where it can be, as multiplying by its inverse
    /// is not: 25.4 mm would show as 0.9999999999999999 in.
    #[must_use]
    pub const fn millimetres(self) -> f64 {
        match self {
            Self::Millimetre => 1.0,
            Self::Centimetre => 10.0,
            Self::Metre => 1000.0,
            Self::Inch => 25.4,
        }
    }
}

impl AngleUnit {
    /// How many of this unit make one radian.
    #[must_use]
    pub const fn per_radian(self) -> f64 {
        match self {
            Self::Radian => 1.0,
            Self::Degree => 180.0 / std::f64::consts::PI,
        }
    }
}

/// The units a caller wants numbers shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayUnits {
    /// The unit for lengths, and for areas and volumes through it.
    pub length: LengthUnit,
    /// The unit for angles.
    pub angle: AngleUnit,
}

impl Default for DisplayUnits {
    /// Occam's internal units.
    fn default() -> Self {
        Self { length: LengthUnit::Millimetre, angle: AngleUnit::Radian }
    }
}
