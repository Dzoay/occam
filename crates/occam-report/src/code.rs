//! Source: original
//! The catalogue: every code occam can report and every remedy it can suggest (L8). Both are closed, so a match on either
//! is exhaustive and a new code cannot be forgotten (L4). `gates errors` holds every code and remedy pair to a test that
//! follows it (L2).

use serde::Serialize;

use crate::Report;
use crate::json::{SCHEMA, encode};

text_enum! {
    /// How serious a report is.
    Severity {
        /// The operation failed.
        Error = "error",
        /// The operation succeeded, with something to be careful of.
        Warning = "warning",
        /// The operation succeeded, and this is worth knowing.
        Note = "note",
    }
}

text_enum! {
    /// A stable identifier for everything occam can report.
    Code {
        /// The text given to [`Report::from_json`] is not a report.
        Malformed = "report.malformed",
        /// The text given to [`Report::from_json`] uses a schema version this build cannot read.
        UnsupportedSchema = "report.unsupported-schema",
        /// A number given as a length or an angle is NaN or infinite, or becomes infinite when converted.
        NotFinite = "units.not-finite",
        /// A tolerance is not a finite number greater than zero.
        ToleranceOutOfRange = "units.tolerance-out-of-range",
        /// An operation was asked for a finer tolerance than the model is built to.
        ToleranceBelowModel = "measure.tolerance-below-model",
        /// What tolerance an operation worked to.
        ToleranceUsed = "measure.tolerance-used",
        /// A command-line argument is missing or is not understood.
        BadArgument = "cli.bad-argument",
    }
}

text_enum! {
    /// A kind of fix occam can suggest. The values to apply it with are in the [`crate::Remedy`].
    RemedyKind {
        /// Read or write the report with the schema version in the `version` parameter.
        UseSchema = "use-schema",
        /// Ask for the tolerance in the `tolerance` parameter, which is the model's.
        UseModelTolerance = "use-model-tolerance",
    }
}

/// What the catalogue says about a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// How serious it is.
    pub severity: Severity,
    /// One sentence on what it means.
    pub summary: &'static str,
    /// The wording, where `{name}` is replaced by the report's value of that name, in the caller's units (L9).
    pub template: &'static str,
    /// The kinds of fix it can suggest.
    pub remedies: &'static [RemedyKind],
}

impl Code {
    /// What the catalogue says about this code.
    #[must_use]
    pub const fn entry(self) -> Entry {
        match self {
            Self::Malformed => Entry {
                severity: Severity::Error,
                summary: "The text is not a report.",
                template: "The text is not a valid report: the problem is at line {line}, column {column}.",
                remedies: &[],
            },
            Self::UnsupportedSchema => Entry {
                severity: Severity::Error,
                summary: "The report uses a schema version this build cannot read.",
                template: "Schema version {found} is not supported. This build reads version {supported}.",
                remedies: &[RemedyKind::UseSchema],
            },
            Self::NotFinite => Entry {
                severity: Severity::Error,
                summary: "A number given as a length or an angle is not finite.",
                template: "The number given is {value}, which is not finite in occam's internal units.",
                remedies: &[],
            },
            Self::ToleranceOutOfRange => Entry {
                severity: Severity::Error,
                summary: "A tolerance must be a finite number greater than zero.",
                template: "The tolerance {tolerance} is not a finite number greater than zero.",
                remedies: &[],
            },
            Self::ToleranceBelowModel => Entry {
                severity: Severity::Error,
                summary: "An operation cannot work to a finer tolerance than its model is built to.",
                template: "The tolerance asked for, {requested}, is finer than the model's tolerance, {model}.",
                remedies: &[RemedyKind::UseModelTolerance],
            },
            Self::ToleranceUsed => Entry {
                severity: Severity::Note,
                summary: "The tolerance an operation worked to.",
                template: "The operation worked to a tolerance of {tolerance}.",
                remedies: &[],
            },
            Self::BadArgument => Entry {
                severity: Severity::Error,
                summary: "A command-line argument is missing or is not understood.",
                template: "Argument {position} is not understood. Expected: occam distance --from X,Y,Z --to X,Y,Z \
                           --model-tolerance LENGTH,ANGLE --tolerance LENGTH [--json], in millimetres and radians.",
                remedies: &[],
            },
        }
    }
}

impl RemedyKind {
    /// One sentence on what following it means.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UseSchema => "Use the schema version given.",
            Self::UseModelTolerance => "Ask for the model's tolerance, or for a coarser one.",
        }
    }

    /// The names of the values it carries.
    #[must_use]
    pub const fn parameters(self) -> &'static [&'static str] {
        match self {
            Self::UseSchema => &["version"],
            Self::UseModelTolerance => &["tolerance"],
        }
    }
}

#[derive(Serialize)]
struct CodeRow {
    name: &'static str,
    code: Code,
    severity: Severity,
    summary: &'static str,
    template: &'static str,
    remedies: &'static [RemedyKind],
}

#[derive(Serialize)]
struct RemedyRow {
    name: &'static str,
    kind: RemedyKind,
    summary: &'static str,
    parameters: &'static [&'static str],
}

#[derive(Serialize)]
struct Catalogue {
    schema: u32,
    codes: Vec<CodeRow>,
    remedies: Vec<RemedyRow>,
}

/// Every code and remedy as JSON (L8).
///
/// # Errors
///
/// A [`Report`] if the catalogue cannot be written, which would be a bug.
pub fn catalogue_json() -> Result<String, Report> {
    let code_row = |code: &Code| {
        let entry = code.entry();
        CodeRow {
            name: code.name(),
            code: *code,
            severity: entry.severity,
            summary: entry.summary,
            template: entry.template,
            remedies: entry.remedies,
        }
    };
    let remedy_row = |kind: &RemedyKind| RemedyRow {
        name: kind.name(),
        kind: *kind,
        summary: kind.summary(),
        parameters: kind.parameters(),
    };
    encode(&Catalogue {
        schema: SCHEMA,
        codes: Code::ALL.iter().map(code_row).collect(),
        remedies: RemedyKind::ALL.iter().map(remedy_row).collect(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn report_and_units_codes() -> Value {
        json!([
            {
                "name": "Malformed",
                "code": "report.malformed",
                "severity": "error",
                "summary": "The text is not a report.",
                "template": "The text is not a valid report: the problem is at line {line}, column {column}.",
                "remedies": [],
            },
            {
                "name": "UnsupportedSchema",
                "code": "report.unsupported-schema",
                "severity": "error",
                "summary": "The report uses a schema version this build cannot read.",
                "template": "Schema version {found} is not supported. This build reads version {supported}.",
                "remedies": ["use-schema"],
            },
            {
                "name": "NotFinite",
                "code": "units.not-finite",
                "severity": "error",
                "summary": "A number given as a length or an angle is not finite.",
                "template": "The number given is {value}, which is not finite in occam's internal units.",
                "remedies": [],
            },
            {
                "name": "ToleranceOutOfRange",
                "code": "units.tolerance-out-of-range",
                "severity": "error",
                "summary": "A tolerance must be a finite number greater than zero.",
                "template": "The tolerance {tolerance} is not a finite number greater than zero.",
                "remedies": [],
            },
        ])
    }

    fn measure_and_command_line_codes() -> Value {
        json!([
            {
                "name": "ToleranceBelowModel",
                "code": "measure.tolerance-below-model",
                "severity": "error",
                "summary": "An operation cannot work to a finer tolerance than its model is built to.",
                "template": "The tolerance asked for, {requested}, is finer than the model's tolerance, {model}.",
                "remedies": ["use-model-tolerance"],
            },
            {
                "name": "ToleranceUsed",
                "code": "measure.tolerance-used",
                "severity": "note",
                "summary": "The tolerance an operation worked to.",
                "template": "The operation worked to a tolerance of {tolerance}.",
                "remedies": [],
            },
            {
                "name": "BadArgument",
                "code": "cli.bad-argument",
                "severity": "error",
                "summary": "A command-line argument is missing or is not understood.",
                "template": "Argument {position} is not understood. Expected: occam distance --from X,Y,Z --to X,Y,Z --model-tolerance LENGTH,ANGLE --tolerance LENGTH [--json], in millimetres and radians.",
                "remedies": [],
            },
        ])
    }

    fn remedies() -> Value {
        json!([
            {
                "name": "UseSchema",
                "kind": "use-schema",
                "summary": "Use the schema version given.",
                "parameters": ["version"],
            },
            {
                "name": "UseModelTolerance",
                "kind": "use-model-tolerance",
                "summary": "Ask for the model's tolerance, or for a coarser one.",
                "parameters": ["tolerance"],
            },
        ])
    }

    #[test]
    fn the_catalogue_lists_every_code_and_remedy() {
        let value: Value = serde_json::from_str(&catalogue_json().unwrap()).unwrap();
        let codes: Vec<Value> = [report_and_units_codes(), measure_and_command_line_codes()]
            .into_iter()
            .filter_map(|rows| rows.as_array().cloned())
            .flatten()
            .collect();
        assert_eq!(value, json!({"schema": 1, "codes": codes, "remedies": remedies()}));
    }

    #[test]
    fn a_remedy_names_the_parameters_it_carries() {
        assert_eq!(RemedyKind::UseSchema.parameters(), ["version"]);
    }

    #[test]
    fn severities_have_stable_text() {
        let texts: Vec<&str> = Severity::ALL.iter().map(|s| s.text()).collect();
        assert_eq!(texts, ["error", "warning", "note"]);
    }
}
