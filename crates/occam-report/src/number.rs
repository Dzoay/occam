//! Source: original
//! A number as JSON is written throughout occam: JSON's own where it is finite, and `"nan"`, `"inf"` or `"-inf"` where
//! it is not, since a solver's residual can be either and an unbounded line's range is infinite. Reports and the native
//! format write theirs alike.

use serde::de::{Deserializer, Error, Visitor};
use serde::{Deserialize, Serialize, Serializer};

/// One number, written so that every finite one reads back exactly and the others by name.
#[derive(Clone, Copy, Debug)]
pub struct Number(pub f64);

impl PartialEq for Number {
    /// Bit for bit, so that a NaN equals itself and `-0` does not equal `0`.
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

/// The number named `text`, if it is one of the three names.
pub(crate) fn named(text: &str) -> Option<f64> {
    match text {
        "nan" => Some(f64::NAN),
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

impl Serialize for Number {
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

/// Reads a number as JSON's own or by one of the three names, without buffering it first: a model holds millions.
struct Visit;

impl Visitor<'_> for Visit {
    type Value = Number;

    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a number, or \"nan\", \"inf\" or \"-inf\"")
    }

    fn visit_f64<E: Error>(self, x: f64) -> Result<Number, E> {
        Ok(Number(x))
    }

    fn visit_u64<E: Error>(self, x: u64) -> Result<Number, E> {
        #[expect(clippy::cast_precision_loss, reason = "as JSON reads an integer into a double")]
        let x = x as f64;
        Ok(Number(x))
    }

    fn visit_i64<E: Error>(self, x: i64) -> Result<Number, E> {
        #[expect(clippy::cast_precision_loss, reason = "as JSON reads an integer into a double")]
        let x = x as f64;
        Ok(Number(x))
    }

    fn visit_str<E: Error>(self, text: &str) -> Result<Number, E> {
        named(text).map(Number).ok_or_else(|| E::custom(format!("not a number: {text}")))
    }
}

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Visit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_number_reads_back_bit_for_bit_and_the_ones_json_lacks_by_name() {
        let numbers = [0.1, -0.0, 0.0, 1e-300, f64::MAX, f64::MIN_POSITIVE / 4.0, f64::INFINITY, f64::NEG_INFINITY];
        let written: Vec<Number> = numbers.iter().copied().map(Number).collect();
        let text = serde_json::to_string(&written).unwrap();
        assert_eq!(text, "[0.1,-0.0,0.0,1e-300,1.7976931348623157e+308,5.562684646268003e-309,\"inf\",\"-inf\"]");
        assert_eq!(serde_json::from_str::<Vec<Number>>(&text).unwrap(), written);
        assert!(serde_json::from_str::<Vec<Number>>("[\"nan\"]").unwrap()[0].0.is_nan());
        assert_eq!(serde_json::to_string(&[Number(f64::NAN)]).unwrap(), "[\"nan\"]");
        assert_eq!(serde_json::from_str::<Vec<Number>>("[3, -2]").unwrap(), [Number(3.0), Number(-2.0)]);
        let refused = serde_json::from_str::<Vec<Number>>("[\"lots\"]").unwrap_err().to_string();
        assert!(refused.contains("not a number: lots"), "{refused}");
        assert!(serde_json::from_str::<Vec<Number>>("[true]").is_err());
        assert_ne!(Number(0.0), Number(-0.0));
    }
}
