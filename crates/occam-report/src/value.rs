//! Source: original
//! What a report points at: entities, and numbers tagged by quantity (L1, L9).

use serde::{Deserialize, Serialize};

use crate::DisplayUnits;

text_enum! {
    /// What sort of thing an entity is.
    EntityKind {
        /// A vertex.
        Vertex = "vertex",
        /// An edge.
        Edge = "edge",
        /// A face.
        Face = "face",
        /// A shell.
        Shell = "shell",
        /// A solid.
        Solid = "solid",
        /// A whole model.
        Model = "model",
    }
}

/// A reference to an entity by its stable id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityRef {
    /// What sort of thing it is.
    pub kind: EntityKind,
    /// Its stable id.
    pub id: u64,
}

impl EntityRef {
    /// A reference to the entity of this kind and id.
    #[must_use]
    pub const fn new(kind: EntityKind, id: u64) -> Self {
        Self { kind, id }
    }
}

text_enum! {
    /// What a number measures, which decides how it is shown.
    Quantity {
        /// A length, in millimetres.
        Length = "length",
        /// An angle, in radians.
        Angle = "angle",
        /// An area, in square millimetres.
        Area = "area",
        /// A volume, in cubic millimetres.
        Volume = "volume",
        /// A count of things.
        Count = "count",
        /// A pure number.
        Ratio = "ratio",
    }
}

/// A number with a name and a quantity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedValue {
    /// What it is called in this report.
    pub name: String,
    /// What it measures.
    pub quantity: Quantity,
    /// Its value in occam's internal units.
    #[serde(with = "number")]
    pub value: f64,
}

impl NamedValue {
    /// A value of any quantity.
    #[must_use]
    pub fn new(name: &str, quantity: Quantity, value: f64) -> Self {
        Self { name: name.to_string(), quantity, value }
    }

    /// A length in millimetres.
    #[must_use]
    pub fn length(name: &str, millimetres: f64) -> Self {
        Self::new(name, Quantity::Length, millimetres)
    }

    /// A count of things.
    #[must_use]
    pub fn count(name: &str, count: u32) -> Self {
        Self::new(name, Quantity::Count, f64::from(count))
    }

    /// The value in the caller's units, with its unit.
    #[must_use]
    pub fn shown(&self, units: DisplayUnits) -> String {
        let length = units.length.millimetres();
        let (value, unit) = match self.quantity {
            Quantity::Length => (self.value / length, units.length.text().to_string()),
            Quantity::Angle => (self.value * units.angle.per_radian(), units.angle.text().to_string()),
            Quantity::Area => (self.value / length / length, format!("{}^2", units.length.text())),
            Quantity::Volume => (self.value / length / length / length, format!("{}^3", units.length.text())),
            Quantity::Count | Quantity::Ratio => (self.value, String::new()),
        };
        if unit.is_empty() { format!("{value}") } else { format!("{value} {unit}") }
    }
}

/// A number as JSON. JSON has no NaN or infinity, and a solver's residual can be either, so those are written as text.
mod number {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::trivially_copy_pass_by_ref, reason = "the signature serde's `with` requires")]
    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            serializer.serialize_f64(*value)
        } else if value.is_nan() {
            serializer.serialize_str("nan")
        } else if value.is_sign_positive() {
            serializer.serialize_str("inf")
        } else {
            serializer.serialize_str("-inf")
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Written {
        Number(f64),
        Text(String),
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        match Written::deserialize(deserializer)? {
            Written::Number(value) => Ok(value),
            Written::Text(text) => match text.as_str() {
                "nan" => Ok(f64::NAN),
                "inf" => Ok(f64::INFINITY),
                "-inf" => Ok(f64::NEG_INFINITY),
                other => Err(D::Error::custom(format!("not a number: {other}"))),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AngleUnit, LengthUnit};

    const IN_CM: DisplayUnits = DisplayUnits { length: LengthUnit::Centimetre, angle: AngleUnit::Degree };

    fn value(quantity: Quantity, value: f64) -> NamedValue {
        NamedValue::new("v", quantity, value)
    }

    #[test]
    fn numbers_are_shown_in_internal_units_by_default() {
        let units = DisplayUnits::default();
        assert_eq!(value(Quantity::Length, 2.5).shown(units), "2.5 mm");
        assert_eq!(value(Quantity::Angle, 1.5).shown(units), "1.5 rad");
        assert_eq!(value(Quantity::Area, 3.0).shown(units), "3 mm^2");
        assert_eq!(value(Quantity::Volume, 4.0).shown(units), "4 mm^3");
    }

    #[test]
    fn lengths_areas_and_volumes_follow_the_callers_unit() {
        assert_eq!(value(Quantity::Length, 20.0).shown(IN_CM), "2 cm");
        assert_eq!(value(Quantity::Area, 300.0).shown(IN_CM), "3 cm^2");
        assert_eq!(value(Quantity::Volume, 5000.0).shown(IN_CM), "5 cm^3");
    }

    #[test]
    fn every_length_unit_converts_from_millimetres() {
        let shown = |length| value(Quantity::Length, 1000.0).shown(DisplayUnits { length, angle: AngleUnit::Radian });
        assert_eq!(shown(LengthUnit::Millimetre), "1000 mm");
        assert_eq!(shown(LengthUnit::Centimetre), "100 cm");
        assert_eq!(shown(LengthUnit::Metre), "1 m");
        assert_eq!(value(Quantity::Length, 25.4).shown(DisplayUnits { length: LengthUnit::Inch, ..IN_CM }), "1 in");
    }

    #[test]
    fn angles_follow_the_callers_unit() {
        assert_eq!(value(Quantity::Angle, std::f64::consts::PI).shown(IN_CM), "180 deg");
    }

    #[test]
    fn counts_and_ratios_have_no_unit_and_do_not_convert() {
        assert_eq!(value(Quantity::Count, 3.0).shown(IN_CM), "3");
        assert_eq!(value(Quantity::Ratio, 0.5).shown(IN_CM), "0.5");
    }

    #[test]
    fn constructors_tag_the_quantity() {
        assert_eq!(NamedValue::length("a", 2.0), NamedValue::new("a", Quantity::Length, 2.0));
        assert_eq!(NamedValue::count("b", 3), NamedValue::new("b", Quantity::Count, 3.0));
        assert_eq!(EntityRef::new(EntityKind::Edge, 7), EntityRef { kind: EntityKind::Edge, id: 7 });
    }

    fn round_trip(number: f64) -> f64 {
        let text = serde_json::to_string(&value(Quantity::Ratio, number)).unwrap();
        serde_json::from_str::<NamedValue>(&text).unwrap().value
    }

    #[test]
    fn finite_numbers_round_trip_exactly() {
        for number in [0.0, 1.0, 0.1 + 0.2, 1e-300, 5e-324, 1.797_693_134_862_315_7e308, -2.5] {
            assert_eq!(round_trip(number).to_bits(), number.to_bits(), "{number}");
        }
    }

    #[test]
    fn numbers_json_cannot_hold_are_written_as_text() {
        let text = |number| serde_json::to_string(&value(Quantity::Ratio, number)).unwrap();
        assert_eq!(text(f64::NAN), r#"{"name":"v","quantity":"ratio","value":"nan"}"#);
        assert_eq!(text(f64::INFINITY), r#"{"name":"v","quantity":"ratio","value":"inf"}"#);
        assert_eq!(text(f64::NEG_INFINITY), r#"{"name":"v","quantity":"ratio","value":"-inf"}"#);
        assert!(round_trip(f64::NAN).is_nan());
        assert_eq!(round_trip(f64::INFINITY), f64::INFINITY);
        assert_eq!(round_trip(f64::NEG_INFINITY), f64::NEG_INFINITY);
    }

    #[test]
    fn a_number_written_as_other_text_is_refused() {
        let text = r#"{"name":"v","quantity":"ratio","value":"lots"}"#;
        assert!(serde_json::from_str::<NamedValue>(text).unwrap_err().to_string().contains("not a number: lots"));
    }
}
