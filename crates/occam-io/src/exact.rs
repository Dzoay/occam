//! Source: original
//! A number as the native format writes it: a whole number below 2^53 as JSON writes an integer, `3` rather than `3.0`,
//! and any other as reports write theirs ([`Number`]), the shortest form that reads back bit for bit, with `"nan"`,
//! `"inf"` and `"-inf"` by name. A whole number reads back exactly either way, and a shorter file reads faster. `-0`
//! keeps its sign.

use occam_report::Number;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 2^53: every whole number of smaller size is exact both as a double and as an integer.
const WHOLE: f64 = 9_007_199_254_740_992.0;

/// One number of a record or of a curve or surface.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Exact(pub(crate) f64);

impl Serialize for Exact {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let x = self.0;
        let negative_zero = x.to_bits() == (-0.0_f64).to_bits();
        if x.fract() == 0.0 && x.abs() < WHOLE && !negative_zero {
            #[expect(clippy::cast_possible_truncation, reason = "a whole number below 2^53, exact as an integer")]
            let whole = x as i64;
            serializer.serialize_i64(whole)
        } else {
            Number(x).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Exact {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Number::deserialize(deserializer).map(|n| Self(n.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_numbers_are_written_as_integers_and_every_number_reads_back_bit_for_bit() {
        let numbers =
            [3.0, -2.0, -0.0, 0.0, 0.5, 9_007_199_254_740_991.0, WHOLE, 1e300, f64::INFINITY, f64::NEG_INFINITY];
        let text = serde_json::to_string(&numbers.map(Exact)).unwrap();
        assert_eq!(text, r#"[3,-2,-0.0,0,0.5,9007199254740991,9007199254740992.0,1e+300,"inf","-inf"]"#);
        let back: Vec<Exact> = serde_json::from_str(&text).unwrap();
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&back.iter().map(|e| e.0).collect::<Vec<_>>()), bits(&numbers));
        assert_eq!(serde_json::to_string(&Exact(f64::NAN)).unwrap(), r#""nan""#);
    }
}
