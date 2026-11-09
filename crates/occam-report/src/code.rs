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
    }
}

text_enum! {
    /// A kind of fix occam can suggest. The values to apply it with are in the [`crate::Remedy`].
    RemedyKind {
        /// Read or write the report with the schema version in the `version` parameter.
        UseSchema = "use-schema",
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
        }
    }
}

impl RemedyKind {
    /// One sentence on what following it means.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UseSchema => "Use the schema version given.",
        }
    }

    /// The names of the values it carries.
    #[must_use]
    pub const fn parameters(self) -> &'static [&'static str] {
        match self {
            Self::UseSchema => &["version"],
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

    #[test]
    fn the_catalogue_lists_every_code_and_remedy() {
        let value: Value = serde_json::from_str(&catalogue_json().unwrap()).unwrap();
        let want = json!({
            "schema": 1,
            "codes": [
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
            ],
            "remedies": [
                {
                    "name": "UseSchema",
                    "kind": "use-schema",
                    "summary": "Use the schema version given.",
                    "parameters": ["version"],
                },
            ],
        });
        assert_eq!(value, want);
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
