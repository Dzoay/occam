//! Source: original
//! A run's one JSON form (L4): whether it went through, the tolerance in force and where it came from, each step that ran
//! with what it created, changed and deleted, what it measured, and what each entity it names is, why it stopped if it
//! did, and the notes. Numbers are in millimetres and radians, as every result gives them; the reports' wording, and
//! each value a description or a measure shows, are in the request's units.

use occam_ops::Measured;
use occam_report::{Code, DisplayUnits, EntityRef, NamedValue, Quantity, Report, SCHEMA, Shown, ValueShown};
use occam_topo::Derivation;
use serde::Serialize;
use serde_json::Value;

use super::{Done, Failed, Ran, ToleranceFrom};

#[derive(Serialize)]
struct RanWire<'a> {
    schema: u32,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tolerance: Option<ToleranceWire>,
    steps: Vec<DoneWire<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed: Option<FailedWire>,
    diagnostics: Vec<Value>,
}

#[derive(Serialize)]
struct ToleranceWire {
    length: f64,
    angle: f64,
    from: &'static str,
}

#[derive(Serialize)]
struct DoneWire<'a> {
    step: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<&'a str>,
    operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    made: Option<EntityRef>,
    created: &'a [EntityRef],
    changed: &'a [EntityRef],
    deleted: &'a [EntityRef],
    lineage: Vec<LineageWire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    measured: Option<MeasuredWire<'a>>,
    described: Vec<Shown<'a>>,
}

#[derive(Serialize)]
struct MeasuredWire<'a> {
    target: EntityRef,
    values: Vec<ValueShown<'a>>,
}

#[derive(Serialize)]
struct LineageWire {
    new: EntityRef,
    old: EntityRef,
    how: &'static str,
}

#[derive(Serialize)]
struct FailedWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<u32>,
    reports: Vec<Value>,
}

/// A report in its JSON form, as a value to place in a result's.
pub(crate) fn report_value(report: &Report, units: DisplayUnits) -> Value {
    match report.to_json(units) {
        Ok(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        Err(failed) => Value::String(failed.code.text().to_string()),
    }
}

pub(super) const fn from(from: ToleranceFrom) -> &'static str {
    match from {
        ToleranceFrom::Request => "request",
        ToleranceFrom::Default => "default",
        ToleranceFrom::Model => "model",
    }
}

pub(super) const fn how(how: Derivation) -> &'static str {
    match how {
        Derivation::Split => "split",
        Derivation::Trimmed => "trimmed",
        Derivation::Merged => "merged",
        Derivation::Copied => "copied",
        Derivation::Swept => "swept",
    }
}

fn measured(measured: &Measured, units: DisplayUnits) -> MeasuredWire<'_> {
    MeasuredWire {
        target: measured.target,
        values: measured.values.iter().map(|v| ValueShown::new(v, units)).collect(),
    }
}

fn done(done: &Done, units: DisplayUnits) -> DoneWire<'_> {
    let changes = &done.changes;
    let lineage = changes.lineage.iter().map(|l| LineageWire { new: l.new, old: l.old, how: how(l.how) }).collect();
    DoneWire {
        step: done.step,
        label: done.label.as_deref(),
        operation: done.operation.name(),
        made: done.made,
        created: &changes.created,
        changed: &changes.changed,
        deleted: &changes.deleted,
        lineage,
        measured: done.measured.as_ref().map(|m| measured(m, units)),
        described: done.described.iter().map(|description| description.shown(units)).collect(),
    }
}

fn failed(failed: &Failed, units: DisplayUnits) -> FailedWire {
    FailedWire { step: failed.step, reports: failed.reports.iter().map(|r| report_value(r, units)).collect() }
}

/// The run's one JSON form.
#[must_use]
pub fn ran_json(ran: &Ran) -> String {
    let tolerance = ran.tolerance.map(|(tolerance, source)| ToleranceWire {
        length: tolerance.length().millimetres(),
        angle: tolerance.angle().radians(),
        from: from(source),
    });
    let used = ran.tolerance.map(|(tolerance, _)| {
        Report::note(Code::ToleranceUsed)
            .with_value(NamedValue::length("tolerance", tolerance.length().millimetres()))
            .with_value(NamedValue::new("angle", Quantity::Angle, tolerance.angle().radians()))
    });
    let wire = RanWire {
        schema: SCHEMA,
        ok: ran.failed.is_none(),
        tolerance,
        steps: ran.done.iter().map(|d| done(d, ran.units)).collect(),
        failed: ran.failed.as_ref().map(|f| failed(f, ran.units)),
        diagnostics: used.iter().map(|r| report_value(r, ran.units)).collect(),
    };
    serde_json::to_string(&wire).unwrap_or_default()
}
