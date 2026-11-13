//! Source: original
//! The one JSON form of a report, with a schema version (L4).

use serde::{Deserialize, Serialize};

use crate::{Code, DisplayUnits, EntityRef, Fault, NamedValue, Partial, Reason, Remedy, RemedyKind, Report, Severity};

/// The version of every JSON form here. It changes when a form changes in a way a reader would notice.
pub const SCHEMA: u32 = 2;

/// A report as it is written.
#[derive(Serialize)]
pub(crate) struct ReportWire<'a> {
    schema: u32,
    code: Code,
    severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    fault: Option<Fault>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<Reason>,
    message: String,
    entities: &'a [EntityRef],
    values: &'a [NamedValue],
    remedies: &'a [Remedy],
    #[serde(skip_serializing_if = "Option::is_none")]
    partial: Option<&'a Partial>,
}

impl<'a> ReportWire<'a> {
    pub(crate) fn new(report: &'a Report, units: DisplayUnits) -> Self {
        Self {
            schema: SCHEMA,
            code: report.code,
            severity: report.code.entry().severity,
            fault: report.fault,
            reason: report.reason,
            message: report.message(units),
            entities: &report.entities,
            values: &report.values,
            remedies: &report.remedies,
            partial: report.partial.as_ref(),
        }
    }
}

fn malformed(error: &serde_json::Error) -> Report {
    let position = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Report::new(Code::Malformed, Fault::InvalidInput)
        .with_value(NamedValue::count("line", position(error.line())))
        .with_value(NamedValue::count("column", position(error.column())))
}

pub(crate) fn encode<T: Serialize>(value: &T) -> Result<String, Report> {
    serde_json::to_string(value).map_err(|error| malformed(&error))
}

#[derive(Deserialize)]
struct Versioned {
    schema: u32,
}

fn unsupported(found: u32) -> Report {
    Report::new(Code::UnsupportedSchema, Fault::InvalidInput)
        .with_value(NamedValue::count("found", found))
        .with_value(NamedValue::count("supported", SCHEMA))
        .with_remedy(Remedy::new(RemedyKind::UseSchema).with_value(NamedValue::count("version", SCHEMA)))
}

impl Report {
    /// The one JSON form of this report (L4), with wording in the caller's units.
    ///
    /// # Errors
    ///
    /// A [`Report`] if it cannot be written, which would be a bug.
    pub fn to_json(&self, units: DisplayUnits) -> Result<String, Self> {
        encode(&ReportWire::new(self, units))
    }

    /// Reads a report back. The wording is not read: it comes from the catalogue.
    ///
    /// # Errors
    ///
    /// A [`Report`] of [`Code::Malformed`] if `text` is not a report, or of [`Code::UnsupportedSchema`] if it is one this
    /// build cannot read.
    pub fn from_json(text: &str) -> Result<Self, Self> {
        let Versioned { schema } = serde_json::from_str(text).map_err(|error| malformed(&error))?;
        if schema != SCHEMA {
            return Err(unsupported(schema));
        }
        serde_json::from_str(text).map_err(|error| malformed(&error))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::{AngleUnit, EntityKind, LengthUnit, Stage, followed_remedy};

    fn rich() -> Report {
        let edge = EntityRef::new(EntityKind::Edge, 7, 1);
        Report::new(Code::UnsupportedSchema, Fault::InvalidInput)
            .with_entity(edge)
            .with_value(NamedValue::count("found", 9))
            .with_value(NamedValue::count("supported", 1))
            .with_value(NamedValue::length("gap", 0.1 + 0.2))
            .with_remedy(
                Remedy::new(RemedyKind::UseSchema).with_entity(edge).with_value(NamedValue::count("version", 1)),
            )
            .with_partial(Partial {
                failing: vec![edge, EntityRef::new(EntityKind::Face, 3, 1)],
                values: vec![NamedValue::new("residual", crate::Quantity::Length, 1e-300)],
                stage: Stage::Pairs,
            })
    }

    #[test]
    fn a_report_round_trips_through_json_unchanged() {
        let report = rich();
        let text = report.to_json(DisplayUnits::default()).unwrap();
        assert_eq!(Report::from_json(&text), Ok(report));
    }

    #[test]
    fn a_schema_2_report_round_trips_with_every_new_field() {
        let scratch = EntityRef::new(EntityKind::Section, 0, 0);
        let report = Report::new(Code::UndefinedParameter, Fault::NumericallySensitive)
            .with_reason(Reason::OutsideDomain)
            .with_entity(EntityRef::new(EntityKind::Surface, 3, 2))
            .with_value(NamedValue::point("at", [1.0, f64::INFINITY, -0.5]))
            .with_tolerance_in_force(1e-7)
            .with_partial(Partial { stage: Stage::Evaluate, failing: vec![scratch], values: vec![] });
        let text = report.to_json(DisplayUnits::default()).unwrap();
        for part in [
            r#""schema":2"#,
            r#""fault":"numerically-sensitive""#,
            r#""reason":"outside-domain""#,
            r#""entities":["surface:3/2"]"#,
            r#""value":[1.0,"inf",-0.5]"#,
            r#""name":"tolerance-in-force","quantity":"length","value":1e-7"#,
            r#""stage":"evaluate","failing":["section:0/0"]"#,
        ] {
            assert!(text.contains(part), "{part} in {text}");
        }
        assert_eq!(Report::from_json(&text), Ok(report));
    }

    #[test]
    fn a_note_has_no_fault_and_no_reason() {
        let text = Report::note(Code::ToleranceUsed).to_json(DisplayUnits::default()).unwrap();
        assert!(!text.contains("fault") && !text.contains("reason"), "{text}");
    }

    #[test]
    fn a_report_without_partial_state_round_trips() {
        let report = Report::new(Code::Malformed, Fault::InvalidInput);
        let text = report.to_json(DisplayUnits::default()).unwrap();
        assert!(!text.contains("partial"), "{text}");
        assert_eq!(Report::from_json(&text), Ok(report));
    }

    #[test]
    fn the_json_form_is_stable() {
        let report = Report::new(Code::UnsupportedSchema, Fault::InvalidInput)
            .with_value(NamedValue::count("found", 9))
            .with_value(NamedValue::count("supported", 1))
            .with_remedy(Remedy::new(RemedyKind::UseSchema).with_value(NamedValue::count("version", 1)));
        let value: Value = serde_json::from_str(&report.to_json(DisplayUnits::default()).unwrap()).unwrap();
        let want = json!({
            "schema": 2,
            "code": "report.unsupported-schema",
            "severity": "error",
            "fault": "invalid-input",
            "message": "Schema version 9 is not supported. This build reads version 1.",
            "entities": [],
            "values": [
                {"name": "found", "quantity": "count", "value": 9.0},
                {"name": "supported", "quantity": "count", "value": 1.0},
            ],
            "remedies": [{"kind": "use-schema", "entities": [], "values": [{"name": "version", "quantity": "count", "value": 1.0}]}],
        });
        assert_eq!(value, want);
    }

    #[test]
    fn the_wording_uses_the_callers_units_and_the_values_do_not() {
        let report = Report::new(Code::Malformed, Fault::InvalidInput).with_value(NamedValue::length("line", 20.0));
        let units = DisplayUnits { length: LengthUnit::Centimetre, angle: AngleUnit::Radian };
        let value: Value = serde_json::from_str(&report.to_json(units).unwrap()).unwrap();
        assert!(value["message"].as_str().unwrap().contains("line 2 cm"), "{value}");
        assert_eq!(value["values"][0]["value"], 20.0);
    }

    #[test]
    fn text_that_is_not_a_report_is_refused_with_its_position() {
        let report = Report::from_json("{\n  oops").unwrap_err();
        assert_eq!(report.code, Code::Malformed);
        assert_eq!(report.value("line").and_then(NamedValue::number), Some(2.0));
        assert_eq!(report.value("column").and_then(NamedValue::number), Some(3.0));
    }

    #[test]
    fn json_with_a_schema_but_no_report_is_malformed() {
        assert_eq!(Report::from_json(r#"{"schema":2}"#).map_err(|r| r.code), Err(Code::Malformed));
        let schema_one = r#"{"schema":1,"code":"report.malformed","entities":[],"values":[],"remedies":[]}"#;
        assert_eq!(
            Report::from_json(schema_one).map_err(|r| r.code),
            Err(Code::UnsupportedSchema),
            "schema 1 is refused"
        );
    }

    #[test]
    fn an_unknown_schema_says_which_one_this_build_reads() {
        let report = Report::from_json(r#"{"schema":99}"#).unwrap_err();
        assert_eq!(report.code, Code::UnsupportedSchema);
        assert_eq!(report.value("found"), Some(&NamedValue::count("found", 99)));
        assert_eq!(report.value("supported"), Some(&NamedValue::count("supported", SCHEMA)));
        assert_eq!(
            report.message(DisplayUnits::default()),
            "Schema version 99 is not supported. This build reads version 2."
        );
    }

    #[test]
    fn following_use_schema_reads_the_report() {
        let text = rich().to_json(DisplayUnits::default()).unwrap().replace("\"schema\":2", "\"schema\":99");
        let report = Report::from_json(&text).unwrap_err();
        followed_remedy!(report, Code::UnsupportedSchema, RemedyKind::UseSchema, |remedy: &Remedy| {
            let version = remedy.value("version").and_then(NamedValue::number).unwrap_or(0.0);
            Report::from_json(&text.replace("\"schema\":99", &format!("\"schema\":{version}")))
        });
    }
}
