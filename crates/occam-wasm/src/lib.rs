//! Source: original
//! The browser build: a thin facade over the `occam` crate. A JavaScript caller passes numbers, in millimetres and radians, and
//! requests as JSON text, and gets the one JSON form of the result or the report (L8, L25).

use occam::{Answered, DistanceQuery, Encoding, Exec, distance_json, ran_json};
use wasm_bindgen::prelude::wasm_bindgen;

/// A model in the browser, as the editing worker keeps it (P10). Its requests run as the command line's do: one that goes
/// through and changes the model is kept, and its native bytes, the binary twin, wait to be published; one that stops
/// short leaves the model as it was.
#[wasm_bindgen]
pub struct Session {
    session: occam::Session,
    saved: Option<Vec<u8>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Session {
    /// A session with no model yet: its first request makes one.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self { session: occam::Session::new(Encoding::Binary), saved: None }
    }

    /// The model `native` holds, in either encoding, measured and checked whole.
    ///
    /// # Errors
    ///
    /// The reports refusing it, in the commands' one JSON form.
    pub fn open(native: &[u8]) -> Result<Self, String> {
        let mut session = occam::Session::open(native, &Exec::serial()).map_err(|r| Answered::Refused(r).json())?;
        session.save_as(Encoding::Binary);
        Ok(Self { session, saved: None })
    }

    /// Runs `request`, written as JSON, and gives the run's one JSON form.
    #[must_use]
    pub fn run(&mut self, request: &str) -> String {
        let (ran, saved) = self.session.run(request);
        self.saved = saved;
        ran_json(&ran)
    }

    /// The model's native bytes, the binary twin, if the last run changed it, once: what the editing worker publishes.
    #[must_use]
    pub fn take_saved(&mut self) -> Option<Vec<u8>> {
        self.saved.take()
    }
}

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

    const BOX: &str =
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [{"op": "box", "size": [1, 2, 3]}]}"#;

    #[test]
    fn a_run_that_changes_the_model_gives_its_binary_twin_once() {
        let mut session = Session::new();
        let ran = session.run(BOX);
        assert!(ran.starts_with(r#"{"schema":2,"ok":true,"#), "{ran}");
        let saved = session.take_saved().unwrap();
        assert!(saved.starts_with(b"\x89occam"), "the binary twin");
        assert_eq!(session.take_saved(), None, "once");
        let measured = session.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "measure", "target": "body:0/1"}]}"#,
        );
        assert!(measured.contains(r#""measured":{"target":"body:0/1","values":[{"name":"volume""#), "{measured}");
        assert_eq!(session.take_saved(), None, "a measure changes nothing");
    }

    #[test]
    fn a_saved_model_opens_again_and_bytes_that_are_not_one_are_refused() {
        let mut first = Session::new();
        let _ = first.run(BOX);
        let mut opened = Session::open(&first.take_saved().unwrap()).unwrap();
        let again = opened.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "copy", "target": "body:0/1", "frame": {"origin": [5, 0, 0]}}]}"#,
        );
        assert!(again.contains(r#""made":"body:1/1""#), "{again}");
        let refused = Session::open(b"not a model").err().unwrap();
        let value: serde_json::Value = serde_json::from_str(&refused).unwrap();
        assert_eq!(value["reports"][0]["code"], "io.not-readable");
    }

    #[test]
    fn a_run_that_stops_short_saves_nothing() {
        let mut session = Session::new();
        let ran = session.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "sphere", "radius": -1}]}"#,
        );
        assert!(ran.starts_with(r#"{"schema":2,"ok":false,"#), "{ran}");
        assert_eq!(session.take_saved(), None);
    }

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
