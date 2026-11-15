//! Source: original
//! What the catalogue says about each code, and the whole catalogue as JSON (L8).

use serde::Serialize;

use super::{Code, RemedyKind};
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
