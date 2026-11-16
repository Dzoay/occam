//! Source: original
//! A command's answers in their one JSON form (L4): whether every id was answered, one answer per id or kind in the order
//! asked, each an answer or the report refusing it, and the reports that refused the command whole or that the check
//! found. In words, each answer is a line, and the refused ones are told apart so the command line can send them to
//! standard error. Numbers are in millimetres and radians, as every result gives them.

use occam_report::{Description, DisplayUnits, EntityRef, Origin, Report, SCHEMA};
use serde::Serialize;
use serde_json::{Value, json};

use super::{Answered, PerId};
use crate::run::{Ran, report_value};

#[derive(Serialize)]
struct AnsweredWire {
    schema: u32,
    ok: bool,
    answers: Vec<Value>,
    reports: Vec<Value>,
}

/// One id's answer as a JSON value: what `field` holds for it, or the report refusing it.
pub(crate) fn answer<T>(id: EntityRef, answer: &Result<T, Report>, field: &str, value: impl Fn(&T) -> Value) -> Value {
    let units = DisplayUnits::default();
    match answer {
        Ok(found) => json!({"entity": id, field: value(found)}),
        Err(report) => json!({"entity": id, "report": report_value(report, units)}),
    }
}

/// `ids` as their text forms.
fn texts(ids: &[EntityRef]) -> Vec<String> {
    ids.iter().map(|id| id.text()).collect()
}

/// A line for each of `reports`, each refused.
fn refusals(reports: &[Report], units: DisplayUnits) -> Vec<(bool, String)> {
    reports.iter().map(|report| (false, report.message(units))).collect()
}

/// One id's answer as a line, the answer said by `say`, or the reason it has none; and whether it was answered.
pub(crate) fn line<T>((id, answer): &PerId<T>, say: impl Fn(&T) -> String) -> (bool, String) {
    match answer {
        Ok(found) => (true, say(found)),
        Err(report) => (false, format!("{}: {}", id.text(), report.message(DisplayUnits::default()))),
    }
}

/// What a description says, in a line: its form, its values, the step that made it and its neighbours.
/// Where an entity came from in words, as `occam run` numbers and names a request's steps: "made by step 4, cylinder",
/// a copy's "made by step 5, copy, of body:2/1", or the model's own step where no request named it.
fn made_by(origin: &Origin) -> String {
    let by = match (origin.request_step, &origin.operation) {
        (Some(index), Some(operation)) => format!("made by step {index}, {operation}"),
        _ => format!("made at the model's step {}", origin.step),
    };
    match origin.copy_of {
        Some(body) => format!("{by}, of {}", body.text()),
        None => by,
    }
}

fn description_words(described: &Description) -> String {
    let units = DisplayUnits::default();
    let values: Vec<String> = described.values.iter().map(|v| format!("{} {}", v.name, v.shown(units))).collect();
    let mut parts = Vec::new();
    if !values.is_empty() {
        parts.push(values.join(", "));
    }
    if let Some(origin) = &described.origin {
        parts.push(made_by(origin));
    }
    if !described.adjacent.is_empty() {
        parts.push(format!("adjacent {}", texts(&described.adjacent).join(" ")));
    }
    format!("{} {}: {}", described.entity.text(), described.form, parts.join("; "))
}

impl Answered {
    /// Whether every id or kind asked about was answered, and nothing was found wrong.
    #[must_use]
    pub fn ok(&self) -> bool {
        match self {
            Self::Listed(_) => true,
            Self::Described(answers) => answers.iter().all(|(_, a)| a.is_ok()),
            Self::Adjacent(answers) => answers.iter().all(|(_, a)| a.is_ok()),
            Self::Checked(reports) => reports.is_empty(),
            Self::Refused(_) => false,
        }
    }

    /// The answers' one JSON form.
    #[must_use]
    pub fn json(&self) -> String {
        let units = DisplayUnits::default();
        let (answers, reports): (Vec<Value>, &[Report]) = match self {
            Self::Listed(listed) => {
                let each = |l: &super::Listed| json!({"kind": l.kind.text(), "entities": l.entities});
                (listed.iter().map(each).collect(), &[])
            }
            Self::Described(answers) => {
                let each = |(id, a): &PerId<Description>| answer(*id, a, "description", |d| json!(d.shown(units)));
                (answers.iter().map(each).collect(), &[])
            }
            Self::Adjacent(answers) => {
                let each = |(id, a): &PerId<Vec<EntityRef>>| answer(*id, a, "adjacent", |near| json!(near));
                (answers.iter().map(each).collect(), &[])
            }
            Self::Checked(reports) | Self::Refused(reports) => (Vec::new(), reports),
        };
        let reports = reports.iter().map(|r| report_value(r, units)).collect();
        serde_json::to_string(&AnsweredWire { schema: SCHEMA, ok: self.ok(), answers, reports }).unwrap_or_default()
    }

    /// The answers in words, a line each, with whether it was answered: the command line sends the answered lines to
    /// standard output and the others to standard error.
    #[must_use]
    pub fn words(&self) -> Vec<(bool, String)> {
        let units = DisplayUnits::default();
        match self {
            Self::Listed(listed) => listed
                .iter()
                .map(|l| {
                    let ids = if l.entities.is_empty() { "(none)".to_string() } else { texts(&l.entities).join(" ") };
                    (true, format!("{}: {ids}", l.kind.text()))
                })
                .collect(),
            Self::Described(answers) => answers.iter().map(|a| line(a, description_words)).collect(),
            Self::Adjacent(answers) => {
                let say = |near: &Vec<EntityRef>| texts(near).join(" ");
                answers.iter().map(|a| line(a, |near| format!("{}: {}", a.0.text(), say(near)))).collect()
            }
            Self::Checked(reports) if reports.is_empty() => vec![(true, "The model is sound.".to_string())],
            Self::Checked(reports) | Self::Refused(reports) => refusals(reports, units),
        }
    }
}

/// A run in words, a line each, with whether it went well: each step that ran, with what it made, how many entities it
/// created, changed and deleted, and what it measured; then why the run stopped, if it did.
#[must_use]
pub fn ran_words(ran: &Ran) -> Vec<(bool, String)> {
    let mut lines: Vec<(bool, String)> = ran.done.iter().map(|done| (true, done_words(done, ran.units))).collect();
    if let Some(failed) = &ran.failed {
        let at = failed.step.map_or_else(String::new, |step| format!("step {step} failed: "));
        lines.extend(failed.reports.iter().map(|report| (false, format!("{at}{}", report.message(ran.units)))));
    }
    lines
}

/// What one step did, in a line.
fn done_words(done: &crate::Done, units: DisplayUnits) -> String {
    let label = done.label.as_ref().map_or_else(String::new, |label| format!(" \"{label}\""));
    let mut parts = Vec::new();
    if let Some(made) = done.made {
        parts.push(format!("made {}", made.text()));
    }
    if done.measured.is_none() {
        let counts = &done.changes;
        let (made, kept, gone) = (counts.created.len(), counts.changed.len(), counts.deleted.len());
        parts.push(format!("created {made}, changed {kept}, deleted {gone}"));
    }
    if let Some(measured) = &done.measured {
        let values: Vec<String> = measured.values.iter().map(|v| format!("{} {}", v.name, v.shown(units))).collect();
        parts.push(format!("{}: {}", measured.target.text(), values.join(", ")));
    }
    format!("step {} {}{label}: {}", done.step, done.operation.name(), parts.join("; "))
}
