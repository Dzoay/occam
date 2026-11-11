//! Source: original
//! Results that can say more than yes or no: a success with diagnostics (L7), and one result per item of a batch (L6).

use serde::Serialize;

use crate::json::{ReportWire, SCHEMA, encode};
use crate::{DisplayUnits, Report};

/// A success, and what occam has to say about it (L7).
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome<T> {
    /// What the operation produced.
    pub value: T,
    /// Anything worth knowing about how it got there.
    pub diagnostics: Vec<Report>,
}

impl<T> Outcome<T> {
    /// A success with nothing to remark on.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value, diagnostics: Vec::new() }
    }

    /// Adds something worth knowing.
    #[must_use]
    pub fn with_diagnostic(mut self, report: Report) -> Self {
        self.diagnostics.push(report);
        self
    }
}

/// One result per item, so filleting 20 edges gives 20 outcomes (L6).
#[derive(Clone, Debug, PartialEq)]
pub struct Batch<T> {
    /// The result of each item, in order.
    pub items: Vec<Result<Outcome<T>, Report>>,
}

impl<T> Batch<T> {
    /// How many items succeeded.
    #[must_use]
    pub fn succeeded(&self) -> usize {
        self.items.iter().filter(|item| item.is_ok()).count()
    }

    /// How many items failed.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.items.iter().filter(|item| item.is_err()).count()
    }
}

fn wires(reports: &[Report], units: DisplayUnits) -> Vec<ReportWire<'_>> {
    reports.iter().map(|report| ReportWire::new(report, units)).collect()
}

#[derive(Serialize)]
struct OutcomeWire<'a, T> {
    schema: u32,
    value: &'a T,
    diagnostics: Vec<ReportWire<'a>>,
}

impl<T: Serialize> Outcome<T> {
    /// The one JSON form of an outcome (L4), with wording in the caller's units.
    ///
    /// # Errors
    ///
    /// A [`Report`] if `value` cannot be written as JSON.
    pub fn to_json(&self, units: DisplayUnits) -> Result<String, Report> {
        encode(&OutcomeWire { schema: SCHEMA, value: &self.value, diagnostics: wires(&self.diagnostics, units) })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum ItemWire<'a, T> {
    Succeeded { index: usize, ok: bool, value: &'a T, diagnostics: Vec<ReportWire<'a>> },
    Failed { index: usize, ok: bool, report: ReportWire<'a> },
}

#[derive(Serialize)]
struct BatchWire<'a, T> {
    schema: u32,
    succeeded: usize,
    failed: usize,
    items: Vec<ItemWire<'a, T>>,
}

fn item_wire<T>(index: usize, item: &Result<Outcome<T>, Report>, units: DisplayUnits) -> ItemWire<'_, T> {
    match item {
        Ok(outcome) => ItemWire::Succeeded {
            index,
            ok: true,
            value: &outcome.value,
            diagnostics: wires(&outcome.diagnostics, units),
        },
        Err(report) => ItemWire::Failed { index, ok: false, report: ReportWire::new(report, units) },
    }
}

impl<T: Serialize> Batch<T> {
    /// The one JSON form of a batch (L4), one entry per item, with wording in the caller's units.
    ///
    /// # Errors
    ///
    /// A [`Report`] if a value cannot be written as JSON.
    pub fn to_json(&self, units: DisplayUnits) -> Result<String, Report> {
        let items = self.items.iter().enumerate().map(|(index, item)| item_wire(index, item, units)).collect();
        encode(&BatchWire { schema: SCHEMA, succeeded: self.succeeded(), failed: self.failed(), items })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::{Code, Fault, NamedValue};

    fn note() -> Report {
        Report::new(Code::Malformed, Fault::InvalidInput)
            .with_value(NamedValue::count("line", 3))
            .with_value(NamedValue::count("column", 4))
    }

    const NOTE_WORDS: &str = "The text is not a valid report: the problem is at line 3, column 4.";

    fn note_json() -> Value {
        json!({
            "schema": 2,
            "code": "report.malformed",
            "severity": "error",
            "fault": "invalid-input",
            "message": NOTE_WORDS,
            "entities": [],
            "values": [
                {"name": "line", "quantity": "count", "value": 3.0},
                {"name": "column", "quantity": "count", "value": 4.0},
            ],
            "remedies": [],
        })
    }

    #[test]
    fn a_success_can_report_too() {
        let outcome = Outcome::new(5).with_diagnostic(note());
        assert_eq!(outcome.diagnostics, [note()]);
        let value: Value = serde_json::from_str(&outcome.to_json(DisplayUnits::default()).unwrap()).unwrap();
        assert_eq!(value, json!({"schema": 2, "value": 5, "diagnostics": [note_json()]}));
    }

    fn batch() -> Batch<u8> {
        Batch { items: vec![Ok(Outcome::new(5)), Err(note()), Ok(Outcome::new(6).with_diagnostic(note()))] }
    }

    #[test]
    fn a_batch_counts_each_kind_of_result() {
        assert_eq!((batch().succeeded(), batch().failed()), (2, 1));
    }

    #[test]
    fn a_batch_with_no_failures_counts_none() {
        let all_good = Batch { items: vec![Ok(Outcome::new(5))] };
        assert_eq!((all_good.succeeded(), all_good.failed()), (1, 0));
    }

    #[test]
    fn a_batch_has_one_entry_per_item_in_order() {
        let value: Value = serde_json::from_str(&batch().to_json(DisplayUnits::default()).unwrap()).unwrap();
        let want = json!({
            "schema": 2,
            "succeeded": 2,
            "failed": 1,
            "items": [
                {"index": 0, "ok": true, "value": 5, "diagnostics": []},
                {"index": 1, "ok": false, "report": note_json()},
                {"index": 2, "ok": true, "value": 6, "diagnostics": [note_json()]},
            ],
        });
        assert_eq!(value, want);
    }
}
