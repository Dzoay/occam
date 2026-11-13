//! Source: original
//! What a report points at: entities, and numbers tagged by quantity (L1, L9).

use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::DisplayUnits;

text_enum! {
    /// What sort of thing an entity is: the topology's kinds, geometry named with no topology (S4), and the parts of a
    /// failed operation's output.
    EntityKind {
        /// A vertex.
        Vertex = "vertex",
        /// An edge.
        Edge = "edge",
        /// One side's use of an edge.
        EdgeUse = "edge-use",
        /// A loop of edge uses bounding a face.
        Loop = "loop",
        /// A wire.
        Wire = "wire",
        /// A face.
        Face = "face",
        /// A shell.
        Shell = "shell",
        /// A body: a solid, a sheet or a wire.
        Body = "body",
        /// A whole model.
        Model = "model",
        /// A surface, named with no face.
        Surface = "surface",
        /// A curve, named with no edge.
        Curve = "curve",
        /// A curve in a face's parameter space.
        ParamCurve = "param-curve",
        /// A section curve that exists only in a failed operation's output.
        Section = "section",
        /// A region that exists only in a failed operation's output.
        Region = "region",
    }
}

/// A reference to an entity: its kind, its slot, and the generation of that slot. Generation 0 is a failed operation's
/// output, whose slot indexes that stage's output. One text form, `edge:41/1`, serves the command line, the JSON and the
/// messages alike: four agents in ten could not read a packed id (S6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityRef {
    /// What sort of thing it is.
    pub kind: EntityKind,
    /// Its slot in the table of its kind.
    pub slot: u32,
    /// How many times that slot has been used, so that an old reference is told from a new one.
    pub generation: u32,
}

impl EntityRef {
    /// A reference to the entity of this kind in this slot and generation.
    #[must_use]
    pub const fn new(kind: EntityKind, slot: u32, generation: u32) -> Self {
        Self { kind, slot, generation }
    }

    /// Whether it exists only in a failed operation's output.
    #[must_use]
    pub const fn is_scratch(self) -> bool {
        self.generation == 0
    }

    /// The one text form: `edge:41/1`.
    #[must_use]
    pub fn text(self) -> String {
        format!("{}:{}/{}", self.kind.text(), self.slot, self.generation)
    }

    /// Reads the text form back, or `None` if it is not one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (kind, numbers) = text.split_once(':')?;
        let (slot, generation) = numbers.split_once('/')?;
        let kind = EntityKind::ALL.iter().copied().find(|candidate| candidate.text() == kind)?;
        let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit()).then(|| part.parse().ok()).flatten();
        Some(Self::new(kind, digits(slot)?, digits(generation)?))
    }
}

impl Serialize for EntityRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text())
    }
}

impl<'de> Deserialize<'de> for EntityRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| D::Error::custom(format!("not an entity reference: {text}")))
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

/// What a named value holds: one number, or a point's three coordinates, so that a position is one value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// One number.
    Scalar(f64),
    /// Three coordinates.
    Point([f64; 3]),
}

/// A number or a point with a name and a quantity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedValue {
    /// What it is called in this report.
    pub name: String,
    /// What it measures; a point's coordinates are lengths.
    pub quantity: Quantity,
    /// Its value in occam's internal units.
    #[serde(with = "number")]
    pub value: Value,
}

impl NamedValue {
    /// A number of any quantity.
    #[must_use]
    pub fn new(name: &str, quantity: Quantity, value: f64) -> Self {
        Self { name: name.to_string(), quantity, value: Value::Scalar(value) }
    }

    /// A point, in millimetres.
    #[must_use]
    pub fn point(name: &str, millimetres: [f64; 3]) -> Self {
        Self { name: name.to_string(), quantity: Quantity::Length, value: Value::Point(millimetres) }
    }

    /// The number, if it holds one rather than a point.
    #[must_use]
    pub const fn number(&self) -> Option<f64> {
        match self.value {
            Value::Scalar(number) => Some(number),
            Value::Point(_) => None,
        }
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

    /// The value in the caller's units, with its unit; a point as `(x, y, z) mm`.
    #[must_use]
    pub fn shown(&self, units: DisplayUnits) -> String {
        let length = units.length.millimetres();
        let number = |value: f64| match self.quantity {
            Quantity::Length => value / length,
            Quantity::Angle => value * units.angle.per_radian(),
            Quantity::Area => value / length / length,
            Quantity::Volume => value / length / length / length,
            Quantity::Count | Quantity::Ratio => value,
        };
        let unit = match self.quantity {
            Quantity::Length => units.length.text().to_string(),
            Quantity::Angle => units.angle.text().to_string(),
            Quantity::Area => format!("{}^2", units.length.text()),
            Quantity::Volume => format!("{}^3", units.length.text()),
            Quantity::Count | Quantity::Ratio => String::new(),
        };
        let text = match self.value {
            Value::Scalar(value) => format!("{}", number(value)),
            Value::Point([x, y, z]) => format!("({}, {}, {})", number(x), number(y), number(z)),
        };
        if unit.is_empty() { text } else { format!("{text} {unit}") }
    }
}

/// A number as JSON. JSON has no NaN or infinity, and a solver's residual can be either, so those are written as text.
mod number {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    use serde::Serialize;
    use serde::ser::SerializeTuple;

    use super::Value;

    /// One number as it is written.
    struct One(f64);

    impl serde::Serialize for One {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            if self.0.is_finite() {
                serializer.serialize_f64(self.0)
            } else if self.0.is_nan() {
                serializer.serialize_str("nan")
            } else if self.0.is_sign_positive() {
                serializer.serialize_str("inf")
            } else {
                serializer.serialize_str("-inf")
            }
        }
    }

    pub fn serialize<S: Serializer>(value: &Value, serializer: S) -> Result<S::Ok, S::Error> {
        match *value {
            Value::Scalar(number) => One(number).serialize(serializer),
            Value::Point(coordinates) => {
                let mut tuple = serializer.serialize_tuple(3)?;
                for coordinate in coordinates {
                    tuple.serialize_element(&One(coordinate))?;
                }
                tuple.end()
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Single {
        Number(f64),
        Text(String),
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Written {
        Single(Single),
        Point([Single; 3]),
    }

    fn read<E: Error>(single: Single) -> Result<f64, E> {
        match single {
            Single::Number(value) => Ok(value),
            Single::Text(text) => match text.as_str() {
                "nan" => Ok(f64::NAN),
                "inf" => Ok(f64::INFINITY),
                "-inf" => Ok(f64::NEG_INFINITY),
                other => Err(E::custom(format!("not a number: {other}"))),
            },
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
        match Written::deserialize(deserializer)? {
            Written::Single(single) => Ok(Value::Scalar(read(single)?)),
            Written::Point([x, y, z]) => Ok(Value::Point([read(x)?, read(y)?, read(z)?])),
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
        assert_eq!(
            EntityRef::new(EntityKind::Edge, 7, 2),
            EntityRef { kind: EntityKind::Edge, slot: 7, generation: 2 }
        );
        assert_eq!(NamedValue::point("p", [1.0, 2.0, 3.0]).value, Value::Point([1.0, 2.0, 3.0]));
        assert_eq!(NamedValue::point("p", [1.0, 2.0, 3.0]).number(), None);
        assert_eq!(NamedValue::length("a", 2.0).number(), Some(2.0));
    }

    fn round_trip(number: f64) -> f64 {
        let text = serde_json::to_string(&value(Quantity::Ratio, number)).unwrap();
        serde_json::from_str::<NamedValue>(&text).unwrap().number().unwrap()
    }

    #[test]
    fn a_point_is_one_value_shown_and_written_as_three() {
        let p = NamedValue::point("at", [10.0, -20.0, f64::NAN]);
        assert_eq!(p.shown(IN_CM), "(1, -2, NaN) cm");
        let text = serde_json::to_string(&p).unwrap();
        assert_eq!(text, r#"{"name":"at","quantity":"length","value":[10.0,-20.0,"nan"]}"#);
        let back: NamedValue = serde_json::from_str(&text).unwrap();
        let Value::Point([x, y, z]) = back.value else { panic!("{back:?}") };
        assert_eq!((x, y, z.is_nan()), (10.0, -20.0, true));
    }

    #[test]
    fn an_entity_reference_has_one_text_form() {
        let edge = EntityRef::new(EntityKind::Edge, 41, 1);
        assert_eq!(edge.text(), "edge:41/1");
        assert_eq!(serde_json::to_string(&edge).unwrap(), r#""edge:41/1""#);
        assert_eq!(
            serde_json::from_str::<EntityRef>(r#""param-curve:0/3""#).unwrap(),
            EntityRef::new(EntityKind::ParamCurve, 0, 3)
        );
        for kind in EntityKind::ALL {
            let reference = EntityRef::new(*kind, 7, 2);
            assert_eq!(EntityRef::parse(&reference.text()), Some(reference), "{kind:?}");
        }
        assert!(EntityRef::new(EntityKind::Region, 3, 0).is_scratch());
        assert!(!edge.is_scratch());
    }

    #[test]
    fn text_that_is_not_an_entity_reference_is_refused() {
        for bad in
            ["edge:41", "edge41/1", "beam:1/1", "edge:-1/1", "edge:1/+1", "edge:1/1/1", "edge:/1", "edge:4294967296/1"]
        {
            assert_eq!(EntityRef::parse(bad), None, "{bad}");
        }
        assert!(
            serde_json::from_str::<EntityRef>(r#""edge:x/1""#)
                .unwrap_err()
                .to_string()
                .contains("not an entity reference")
        );
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
