//! Source: original
//! Reading the harness's result files, in the format `tools/oracle/README.md` defines. A file that cannot be trusted counts as no
//! result, and a case the oracle could not do is not a failure of occam's.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::compare::{Disagreement, Measured, Tolerances, compare};
use crate::locate::Pin;

/// Why there is nothing to compare with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoResult {
    /// There is no file for this case.
    Missing,
    /// The file is not a result.
    Malformed,
    /// The file has another schema.
    WrongSchema,
    /// The file came from another engine or version.
    WrongEngine,
}

/// What the oracle says about a case.
#[derive(Clone, Debug, PartialEq)]
pub enum Reading {
    /// It made a shape and measured it.
    Shape(Measured),
    /// It could not do this.
    CouldNot {
        /// The harness's code for what went wrong.
        code: String,
        /// The id of the operation at fault, if known.
        op: Option<String>,
    },
    /// There is nothing to compare with.
    Nothing(NoResult),
}

/// The result of comparing occam's measurements with a reading.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// They agree within tolerance.
    Agrees,
    /// They differ on these measures.
    Disagrees(Vec<Disagreement>),
    /// The oracle could not do this, which says nothing about occam.
    OracleCouldNot,
    /// There is nothing to compare with.
    Nothing(NoResult),
}

const SCHEMA: &str = "occam-oracle-result/1";

#[derive(Deserialize)]
struct Head {
    schema: String,
}

#[derive(Deserialize)]
struct OracleWire {
    engine: String,
    engine_version: String,
    package: String,
    package_version: String,
}

impl OracleWire {
    fn matches(&self, pin: Pin) -> bool {
        self.engine == pin.engine
            && self.engine_version == pin.engine_version
            && self.package == pin.package
            && self.package_version == pin.package_version
    }
}

#[derive(Deserialize)]
struct BoxWire {
    min: [f64; 3],
    max: [f64; 3],
}

/// The measures that are compared. `counts` is absent on purpose (L12).
#[derive(Deserialize)]
struct ShapeWire {
    volume: f64,
    area: f64,
    bbox: Option<BoxWire>,
    euler: i64,
    valid: bool,
    closed: bool,
}

impl From<ShapeWire> for Measured {
    fn from(shape: ShapeWire) -> Self {
        Self {
            volume: shape.volume,
            area: shape.area,
            bbox: shape.bbox.map(|bbox| (bbox.min, bbox.max)),
            euler: shape.euler,
            valid: shape.valid,
            closed: shape.closed,
        }
    }
}

#[derive(Deserialize)]
struct ErrorWire {
    code: String,
    op: Option<String>,
}

#[derive(Deserialize)]
struct Wire {
    oracle: OracleWire,
    status: String,
    shape: Option<ShapeWire>,
    error: Option<ErrorWire>,
}

fn parsed(text: &str, pin: Pin) -> Result<Reading, NoResult> {
    let head: Head = serde_json::from_str(text).map_err(|_| NoResult::Malformed)?;
    if head.schema != SCHEMA {
        return Err(NoResult::WrongSchema);
    }
    let wire: Wire = serde_json::from_str(text).map_err(|_| NoResult::Malformed)?;
    if !wire.oracle.matches(pin) {
        return Err(NoResult::WrongEngine);
    }
    match (wire.status.as_str(), wire.shape, wire.error) {
        ("ok", Some(shape), _) => Ok(Reading::Shape(shape.into())),
        ("error", _, Some(error)) => Ok(Reading::CouldNot { code: error.code, op: error.op }),
        _ => Err(NoResult::Malformed),
    }
}

/// Reads one result file's text, which must have come from `pin`.
#[must_use]
pub fn read(text: &str, pin: Pin) -> Reading {
    parsed(text, pin).unwrap_or_else(Reading::Nothing)
}

/// Reads the result for `case` run at `threads`, under `root`. A file that cannot be read counts as missing.
#[must_use]
pub fn load(root: &Path, pin: Pin, threads: u32, case: &str) -> Reading {
    fs::read_to_string(pin.path(root, threads, case))
        .map_or(Reading::Nothing(NoResult::Missing), |text| read(&text, pin))
}

/// Compares `ours` with what the oracle said (L11, L12).
#[must_use]
pub fn judge(ours: &Measured, reading: &Reading, tolerances: Tolerances) -> Verdict {
    match reading {
        Reading::Shape(oracle) => {
            let found = compare(ours, oracle, tolerances);
            if found.is_empty() { Verdict::Agrees } else { Verdict::Disagrees(found) }
        }
        Reading::CouldNot { .. } => Verdict::OracleCouldNot,
        Reading::Nothing(why) => Verdict::Nothing(*why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::{Closeness, Measure};
    use crate::locate::OPENCASCADE;

    const OK: &str = r#"{
  "id": "a",
  "oracle": {"engine": "opencascade", "engine_version": "8.0.1", "package": "cadquery-ocp", "package_version": "8.0.1.1.0"},
  "schema": "occam-oracle-result/1",
  "shape": {
    "area": 600.0,
    "bbox": {"max": [10.0, 20.0, 30.0], "min": [1.0, 2.0, 3.0]},
    "closed": true,
    "counts": {"degenerate_edges": 0, "edges": 12, "faces": 6, "shells": 1, "solids": 1, "vertices": 8, "wires": 6},
    "euler": 2,
    "valid": false,
    "volume": 1000.0
  },
  "status": "ok",
  "tessellations": {},
  "threads": 1
}"#;

    fn measured() -> Measured {
        Measured {
            volume: 1000.0,
            area: 600.0,
            bbox: Some(([1.0, 2.0, 3.0], [10.0, 20.0, 30.0])),
            euler: 2,
            valid: false,
            closed: true,
        }
    }

    #[test]
    fn a_success_gives_the_measures() {
        assert_eq!(read(OK, OPENCASCADE), Reading::Shape(measured()));
    }

    #[test]
    fn an_empty_result_has_no_box() {
        let text = OK.replace(r#""bbox": {"max": [10.0, 20.0, 30.0], "min": [1.0, 2.0, 3.0]}"#, r#""bbox": null"#);
        assert_eq!(read(&text, OPENCASCADE), Reading::Shape(Measured { bbox: None, ..measured() }));
    }

    #[test]
    fn the_counts_are_not_needed() {
        let text = OK.replace(
            r#""counts": {"degenerate_edges": 0, "edges": 12, "faces": 6, "shells": 1, "solids": 1, "vertices": 8, "wires": 6},"#,
            "",
        );
        assert_eq!(read(&text, OPENCASCADE), Reading::Shape(measured()));
    }

    fn failure(op: &str) -> String {
        let oracle = r#""oracle": {"engine": "opencascade", "engine_version": "8.0.1", "package": "cadquery-ocp", "package_version": "8.0.1.1.0"}"#;
        format!(
            r#"{{"id": "a", {oracle}, "schema": "occam-oracle-result/1", "status": "error", "threads": 1,
                "error": {{"code": "op_failed", "kind": "fillet", "message": "not done", "op": {op}}}}}"#
        )
    }

    #[test]
    fn a_failure_gives_its_code_and_op() {
        let want = Reading::CouldNot { code: "op_failed".to_string(), op: Some("too-big".to_string()) };
        assert_eq!(read(&failure(r#""too-big""#), OPENCASCADE), want);
    }

    #[test]
    fn a_failure_may_name_no_op() {
        let want = Reading::CouldNot { code: "op_failed".to_string(), op: None };
        assert_eq!(read(&failure("null"), OPENCASCADE), want);
    }

    fn nothing(text: &str) -> Reading {
        read(text, OPENCASCADE)
    }

    #[test]
    fn what_is_not_a_result_counts_as_none() {
        let malformed = Reading::Nothing(NoResult::Malformed);
        assert_eq!(nothing("not json"), malformed);
        assert_eq!(nothing("{}"), malformed);
        assert_eq!(nothing(&OK.replace(r#""status": "ok""#, r#""status": "maybe""#)), malformed, "an unknown status");
        let no_shape =
            OK.split_once(r#""shape""#).map_or(String::new(), |(head, _)| format!("{head}\"status\": \"ok\"}}"));
        assert_eq!(nothing(&no_shape), malformed, "a success with no shape");
        let no_error = failure("null").replace(
            r#""error": {"code": "op_failed", "kind": "fillet", "message": "not done", "op": null}"#,
            r#""x": 1"#,
        );
        assert_eq!(nothing(&no_error), malformed, "a failure with no error");
    }

    #[test]
    fn another_schema_counts_as_none() {
        let text = OK.replace("occam-oracle-result/1", "occam-oracle-result/2");
        assert_eq!(nothing(&text), Reading::Nothing(NoResult::WrongSchema));
    }

    #[test]
    fn each_part_of_the_pin_must_match() {
        let wrong = Reading::Nothing(NoResult::WrongEngine);
        let replaced = [
            (r#""engine": "opencascade""#, r#""engine": "other""#),
            (r#""engine_version": "8.0.1""#, r#""engine_version": "9""#),
            (r#""package": "cadquery-ocp""#, r#""package": "other""#),
            (r#""package_version": "8.0.1.1.0""#, r#""package_version": "9""#),
        ];
        for (from, to) in replaced {
            assert_eq!(nothing(&OK.replace(from, to)), wrong, "{to}");
        }
    }

    const STRICT: Closeness = Closeness { rel: 1e-6, abs: 0.0 };
    const TOLERANCES: Tolerances = Tolerances { volume: STRICT, area: STRICT, bbox: 1e-6 };

    #[test]
    fn matching_measures_agree() {
        assert_eq!(judge(&measured(), &Reading::Shape(measured()), TOLERANCES), Verdict::Agrees);
    }

    #[test]
    fn differing_measures_are_listed() {
        let ours = Measured { volume: 1.0, ..measured() };
        let want = Verdict::Disagrees(vec![Disagreement { measure: Measure::Volume }]);
        assert_eq!(judge(&ours, &Reading::Shape(measured()), TOLERANCES), want);
    }

    #[test]
    fn a_case_the_oracle_could_not_do_is_not_a_failure() {
        let reading = Reading::CouldNot { code: "op_failed".to_string(), op: None };
        assert_eq!(judge(&measured(), &reading, TOLERANCES), Verdict::OracleCouldNot);
    }

    #[test]
    fn no_result_is_passed_on() {
        let reading = Reading::Nothing(NoResult::WrongEngine);
        assert_eq!(judge(&measured(), &reading, TOLERANCES), Verdict::Nothing(NoResult::WrongEngine));
    }

    #[cfg(not(target_family = "wasm"))]
    mod files {
        use std::fs;
        use std::path::PathBuf;

        use super::*;

        fn scratch(name: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!("occam-verify-{}-{name}", std::process::id()));
            drop(fs::remove_dir_all(&dir));
            dir
        }

        #[test]
        fn a_case_with_no_file_has_no_result() {
            let root = scratch("absent");
            assert_eq!(load(&root, OPENCASCADE, 1, "a"), Reading::Nothing(NoResult::Missing));
        }

        #[test]
        fn a_file_is_found_where_the_harness_writes_it() {
            let root = scratch("present");
            let path = OPENCASCADE.path(&root, 1, "a");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, OK).unwrap();
            assert_eq!(load(&root, OPENCASCADE, 1, "a"), Reading::Shape(measured()));
            fs::remove_dir_all(&root).unwrap();
        }
    }
}
