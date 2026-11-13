//! Source: original
//! The browser build: a thin facade over the `occam` crate. A JavaScript caller passes numbers, in millimetres and radians, and
//! gets the one JSON form of the result or the report (L8, L25).

use occam::{DistanceQuery, distance_json};
use wasm_bindgen::prelude::wasm_bindgen;

/// The distance between two points, as the JSON form of its result or of the report refusing it. Everything is in millimetres
/// and radians.
#[wasm_bindgen]
#[must_use]
#[allow(clippy::too_many_arguments, reason = "JavaScript passes flat numbers, so the boundary stays free of objects")]
pub fn distance(
    from_x: f64,
    from_y: f64,
    from_z: f64,
    to_x: f64,
    to_y: f64,
    to_z: f64,
    model_length_tolerance: f64,
    model_angle_tolerance: f64,
    tolerance: f64,
) -> String {
    let query = DistanceQuery {
        from: [from_x, from_y, from_z],
        to: [to_x, to_y, to_z],
        model_length_tolerance,
        model_angle_tolerance,
        tolerance,
    };
    distance_json(&query).json
}

#[cfg(test)]
mod tests {
    use occam::{Code, Report};

    use super::*;

    #[test]
    fn every_number_reaches_the_query() {
        let got = distance(1.0, 2.0, 3.0, 4.0, 6.0, 3.0, 0.001, 0.00001, 0.01);
        assert!(got.starts_with(r#"{"schema":2,"value":5.0,"#), "{got}");
    }

    #[test]
    fn a_refusal_is_the_reports_json() {
        let got = distance(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.001, 0.00001, 0.0001);
        assert_eq!(Report::from_json(&got).map(|report| report.code), Ok(Code::ToleranceBelowModel));
    }

    #[test]
    fn the_models_tolerances_are_kept_apart() {
        let got = distance(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.001, 0.0, 0.01);
        assert_eq!(Report::from_json(&got).map(|report| report.code), Ok(Code::ToleranceOutOfRange));
        let got = distance(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.0, 0.00001, 0.01);
        assert_eq!(Report::from_json(&got).map(|report| report.code), Ok(Code::ToleranceOutOfRange));
    }
}
