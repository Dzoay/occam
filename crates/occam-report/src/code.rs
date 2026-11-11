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
        /// The threads asked for could not be started.
        ThreadsUnavailable = "exec.threads-unavailable",
        /// A direction was asked of a vector too short to have one.
        DegenerateDirection = "geom.degenerate-direction",
        /// A curve or surface was asked for a parameter where it is not defined.
        UndefinedParameter = "geom.undefined-parameter",
        /// A curve or surface was given a size that is not a finite number greater than zero.
        SizeNotPositive = "geom.size-not-positive",
        /// A curve or surface was given a size or an angle outside the range its form allows.
        OutOfRange = "geom.out-of-range",
    }
}

text_enum! {
    /// A kind of fix occam can suggest. The values to apply it with are in the [`crate::Remedy`].
    RemedyKind {
        /// Read or write the report with the schema version in the `version` parameter.
        UseSchema = "use-schema",
        /// Ask for the tolerance in the `tolerance` parameter, which is the model's.
        UseModelTolerance = "use-model-tolerance",
        /// Run on the number of threads in the `threads` parameter.
        UseThreads = "use-threads",
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
        Entry {
            severity: self.severity(),
            summary: self.summary(),
            template: self.template(),
            remedies: self.remedies(),
        }
    }

    /// How serious it is.
    const fn severity(self) -> Severity {
        match self {
            Self::Malformed
            | Self::UnsupportedSchema
            | Self::NotFinite
            | Self::ToleranceOutOfRange
            | Self::ToleranceBelowModel
            | Self::BadArgument
            | Self::ThreadsUnavailable
            | Self::DegenerateDirection
            | Self::UndefinedParameter
            | Self::SizeNotPositive
            | Self::OutOfRange => Severity::Error,
            Self::ToleranceUsed => Severity::Note,
        }
    }

    /// One sentence on what it means.
    const fn summary(self) -> &'static str {
        match self {
            Self::Malformed => "The text is not a report.",
            Self::UnsupportedSchema => "The report uses a schema version this build cannot read.",
            Self::NotFinite => "A number given as a length or an angle is not finite.",
            Self::ToleranceOutOfRange => "A tolerance must be a finite number greater than zero.",
            Self::ToleranceBelowModel => "An operation cannot work to a finer tolerance than its model is built to.",
            Self::ToleranceUsed => "The tolerance an operation worked to.",
            Self::BadArgument => "A command-line argument is missing or is not understood.",
            Self::ThreadsUnavailable => "The threads asked for could not be started.",
            Self::DegenerateDirection => "A vector no longer than the model's tolerance has no direction.",
            Self::UndefinedParameter => "A curve or surface is not defined at the parameter asked for.",
            Self::SizeNotPositive => {
                "A curve or surface was given a size that is not a finite number greater than zero."
            }
            Self::OutOfRange => "A curve or surface was given a size or an angle outside the range its form allows.",
        }
    }

    /// The wording, with `{name}` for each value it shows.
    const fn template(self) -> &'static str {
        match self {
            Self::Malformed => "The text is not a valid report: the problem is at line {line}, column {column}.",
            Self::UnsupportedSchema => "Schema version {found} is not supported. This build reads version {supported}.",
            Self::NotFinite => "The number given is {value}, which is not finite in occam's internal units.",
            Self::ToleranceOutOfRange => "The tolerance {tolerance} is not a finite number greater than zero.",
            Self::ToleranceBelowModel => {
                "The tolerance asked for, {requested}, is finer than the model's tolerance, {model}."
            }
            Self::ToleranceUsed => "The operation worked to a tolerance of {tolerance}.",
            Self::BadArgument => {
                "Argument {position} is not understood. Expected: occam distance --from X,Y,Z --to X,Y,Z \
                 --model-tolerance LENGTH,ANGLE --tolerance LENGTH [--json], in millimetres and radians."
            }
            Self::ThreadsUnavailable => "{threads} threads could not be started on this machine.",
            Self::DegenerateDirection => {
                "The vector is {length} long, no longer than the tolerance of {tolerance}, so it has no direction."
            }
            Self::UndefinedParameter => "The parameter {parameter} is outside the domain from {low} to {high}.",
            Self::SizeNotPositive => "The size given, {size}, is not a finite number greater than zero.",
            Self::OutOfRange => "The value given, {value}, is not strictly between {low} and {high}.",
        }
    }

    /// The kinds of fix it can suggest.
    const fn remedies(self) -> &'static [RemedyKind] {
        match self {
            Self::UnsupportedSchema => &[RemedyKind::UseSchema],
            Self::ToleranceBelowModel => &[RemedyKind::UseModelTolerance],
            Self::ThreadsUnavailable => &[RemedyKind::UseThreads],
            Self::Malformed
            | Self::NotFinite
            | Self::ToleranceOutOfRange
            | Self::ToleranceUsed
            | Self::BadArgument
            | Self::DegenerateDirection
            | Self::UndefinedParameter
            | Self::SizeNotPositive
            | Self::OutOfRange => &[],
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
            Self::UseThreads => {
                "Run on the number of threads given. One thread needs none started, so it always works."
            }
        }
    }

    /// The names of the values it carries.
    #[must_use]
    pub const fn parameters(self) -> &'static [&'static str] {
        match self {
            Self::UseSchema => &["version"],
            Self::UseModelTolerance => &["tolerance"],
            Self::UseThreads => &["threads"],
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

    fn measure_command_line_and_exec_codes() -> Value {
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
            {
                "name": "ThreadsUnavailable",
                "code": "exec.threads-unavailable",
                "severity": "error",
                "summary": "The threads asked for could not be started.",
                "template": "{threads} threads could not be started on this machine.",
                "remedies": ["use-threads"],
            },
        ])
    }

    fn geom_codes() -> Value {
        json!([
            {
                "name": "DegenerateDirection",
                "code": "geom.degenerate-direction",
                "severity": "error",
                "summary": "A vector no longer than the model's tolerance has no direction.",
                "template": "The vector is {length} long, no longer than the tolerance of {tolerance}, so it has no direction.",
                "remedies": [],
            },
            {
                "name": "UndefinedParameter",
                "code": "geom.undefined-parameter",
                "severity": "error",
                "summary": "A curve or surface is not defined at the parameter asked for.",
                "template": "The parameter {parameter} is outside the domain from {low} to {high}.",
                "remedies": [],
            },
            {
                "name": "SizeNotPositive",
                "code": "geom.size-not-positive",
                "severity": "error",
                "summary": "A curve or surface was given a size that is not a finite number greater than zero.",
                "template": "The size given, {size}, is not a finite number greater than zero.",
                "remedies": [],
            },
            {
                "name": "OutOfRange",
                "code": "geom.out-of-range",
                "severity": "error",
                "summary": "A curve or surface was given a size or an angle outside the range its form allows.",
                "template": "The value given, {value}, is not strictly between {low} and {high}.",
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
            {
                "name": "UseThreads",
                "kind": "use-threads",
                "summary": "Run on the number of threads given. One thread needs none started, so it always works.",
                "parameters": ["threads"],
            },
        ])
    }

    #[test]
    fn the_catalogue_lists_every_code_and_remedy() {
        let value: Value = serde_json::from_str(&catalogue_json().unwrap()).unwrap();
        let codes: Vec<Value> = [report_and_units_codes(), measure_command_line_and_exec_codes(), geom_codes()]
            .into_iter()
            .filter_map(|rows| rows.as_array().cloned())
            .flatten()
            .collect();
        assert_eq!(value, json!({"schema": 2, "codes": codes, "remedies": remedies()}));
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
