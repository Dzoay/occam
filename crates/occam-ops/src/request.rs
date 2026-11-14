//! Source: original
//! A request, read and checked whole before any geometry (docs/architecture/operations.md): its schema version, the units
//! its numbers are in, an optional tolerance for a new model, and its steps, each with an optional label and the operation
//! it asks for. Every bad input is reported, each with its own code and its place in the request (S6), so an agent fixes
//! them all at once.

use std::collections::BTreeMap;

use occam_report::{AngleUnit, Code, EntityRef, Fault, LengthUnit, NamedValue, Remedy, RemedyKind, Report};
use occam_units::{Angle, Length, Tolerance};
use serde_json::{Map, Value};

use crate::args::{self, Args};
use crate::operation::Operation;
use crate::read::{At, count, name, needed, number, object, text, whole, without_knock_ons, wrong_type};

/// The version of the request's JSON form this build reads.
pub const SCHEMA: u32 = 1;

/// The units a request's numbers are in. Converted once, here; everything after works in millimetres and radians.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Units {
    /// Lengths', and coordinates'.
    pub length: LengthUnit,
    /// Angles'.
    pub angle: AngleUnit,
}

/// One step of a request.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// Its index in the request, from 0.
    pub index: u32,
    /// The name later steps can give what it makes.
    pub label: Option<String>,
    /// What it asks for.
    pub operation: Operation,
    /// Its other fields, which its operation reads.
    pub parameters: Map<String, Value>,
    /// What its operation reads of them, checked and converted.
    pub args: Args,
}

/// A request, checked and converted.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// The units its numbers were in.
    pub units: Units,
    /// The tolerance it asks a new model to be built to, if it gives one.
    pub tolerance: Option<Tolerance>,
    /// Its steps, in order.
    pub steps: Vec<Step>,
}

/// The tolerance a new model is built to when its request gives none (O12): 1e-7 mm and 1e-9 rad.
///
/// # Errors
///
/// None: both are finite and greater than zero.
pub fn default_tolerance() -> Result<Tolerance, Report> {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9))
}

/// The names of `all`, as the reports give them, each with its value.
fn names<T: Copy>(all: &[T], text: impl Fn(T) -> &'static str) -> Vec<(&'static str, T)> {
    all.iter().map(|&unit| (text(unit), unit)).collect()
}

impl Request {
    /// The request written as `text`, checked whole.
    ///
    /// # Errors
    ///
    /// [`Code::RequestMalformed`] alone for text that is not JSON, and a report about the schema version alone if it is
    /// missing or not this build's. Otherwise every bad input, each with its own code and its place in the request, field
    /// by field: fields the request does not take, then its units, its tolerance, and its steps in order.
    pub fn parse(text: &str) -> Result<Self, Vec<Report>> {
        Self::read(text).map_err(|unread| unread.reports)
    }

    /// The request written as `text`, checked whole as [`Self::parse`] checks it.
    ///
    /// # Errors
    ///
    /// What [`Self::parse`] reports, with the units the request's numbers are in if its `units` could be read.
    pub fn read(text: &str) -> Result<Self, Unread> {
        let alone = |report: Report| Unread { units: None, reports: vec![report] };
        let value: Value = serde_json::from_str(text).map_err(|error| alone(malformed(&error)))?;
        let mut out = Vec::new();
        let root = At::root();
        let Some(fields) = object(&root, &value, &["schema", "units", "tolerance", "steps"], &mut out) else {
            return Err(Unread { units: None, reports: out });
        };
        schema(&root, fields).map_err(alone)?;
        let units = units(&root, fields, &mut out);
        let tolerance = match (units, fields.get("tolerance")) {
            (Some(units), Some(given)) => tolerance(&root.field("tolerance"), given, units, &mut out),
            (None | Some(_), _) => None,
        };
        let read_in = units.unwrap_or(Units { length: LengthUnit::Millimetre, angle: AngleUnit::Radian });
        let steps = steps(&root, fields, read_in, &mut out);
        match (out.is_empty(), units, steps) {
            (true, Some(units), Some(steps)) => Ok(Self { units, tolerance, steps }),
            _ => Err(Unread { units, reports: without_knock_ons(out) }),
        }
    }
}

/// A request refused when it was read: what was wrong, and the units its numbers are in if its `units` could be read,
/// which the reports are worded in.
#[derive(Clone, Debug, PartialEq)]
pub struct Unread {
    /// The units, if they could be read.
    pub units: Option<Units>,
    /// Every bad input, each with its own code and its place in the request.
    pub reports: Vec<Report>,
}

/// The value of `result`, or nothing with its report kept.
fn gather<T>(result: Result<T, Report>, out: &mut Vec<Report>) -> Option<T> {
    result.map_err(|report| out.push(report)).ok()
}

/// Text that is not JSON, and where.
fn malformed(error: &serde_json::Error) -> Report {
    Report::new(Code::RequestMalformed, Fault::InvalidInput)
        .with_value(count("line", error.line()))
        .with_value(count("column", error.column()))
}

/// The request's schema version, which must be this build's.
fn schema(root: &At, fields: &Map<String, Value>) -> Result<(), Report> {
    let at = root.field("schema");
    let found = whole(&at, needed(root, fields, "schema")?)?;
    if found == u64::from(SCHEMA) {
        return Ok(());
    }
    let found = u32::try_from(found).unwrap_or(u32::MAX);
    Err(at
        .report(Code::RequestUnsupportedSchema)
        .with_value(NamedValue::count("found", found))
        .with_value(NamedValue::count("supported", SCHEMA))
        .with_remedy(Remedy::new(RemedyKind::UseSchema).with_value(NamedValue::count("version", SCHEMA))))
}

/// The units the request's numbers are in, with every bad input in `out`.
fn units(root: &At, fields: &Map<String, Value>, out: &mut Vec<Report>) -> Option<Units> {
    let at = root.field("units");
    let given = gather(needed(root, fields, "units"), out)?;
    let given = object(&at, given, &["length", "angle"], out)?;
    let (lengths, angles) = (names(LengthUnit::ALL, LengthUnit::text), names(AngleUnit::ALL, AngleUnit::text));
    let (length, angle) = (unit(&at, given, "length", &lengths), unit(&at, given, "angle", &angles));
    let (length, angle) = (gather(length, out), gather(angle, out));
    Some(Units { length: length?, angle: angle? })
}

/// The unit the field `field` of `given`, at `at`, names: one of `units`, by its short name.
fn unit<T: Copy>(at: &At, given: &Map<String, Value>, field: &str, units: &[(&str, T)]) -> Result<T, Report> {
    let value = needed(at, given, field)?;
    written_out(&at.field(field), value)?;
    name(&at.field(field), value, units)
}

/// Units written out in full, as people write them, in either spelling and either number, each with the short name a
/// request writes it by.
const WRITTEN_OUT: [(&str, &str); 18] = [
    ("millimetre", "mm"),
    ("millimetres", "mm"),
    ("millimeter", "mm"),
    ("millimeters", "mm"),
    ("centimetre", "cm"),
    ("centimetres", "cm"),
    ("centimeter", "cm"),
    ("centimeters", "cm"),
    ("metre", "m"),
    ("metres", "m"),
    ("meter", "m"),
    ("meters", "m"),
    ("inch", "in"),
    ("inches", "in"),
    ("degree", "deg"),
    ("degrees", "deg"),
    ("radian", "rad"),
    ("radians", "rad"),
];

/// Nothing, or `input.unit-written-out` where the unit at `at` is written out in full, in any case, with the short
/// name to write instead.
fn written_out(at: &At, value: &Value) -> Result<(), Report> {
    let Some(given) = value.as_str() else { return Ok(()) };
    let lower = given.to_lowercase();
    WRITTEN_OUT.iter().find(|(full, _)| *full == lower).map_or(Ok(()), |&(_, short)| {
        let input = at.input().given(given).expected(short);
        Err(Report::new(Code::InputUnitWrittenOut, Fault::InvalidInput).with_input(input))
    })
}

/// The tolerance at `at`, in `units`, for a new model, with every bad input in `out`.
fn tolerance(at: &At, given: &Value, units: Units, out: &mut Vec<Report>) -> Option<Tolerance> {
    let fields = object(at, given, &["length", "angle"], out)?;
    let read = |name: &str| needed(at, fields, name).and_then(|value| number(&at.field(name), value));
    let (length, angle) = (gather(read("length"), out), gather(read("angle"), out));
    let placed = |name: &'static str| move |report: Report| report.with_input(at.field(name).input());
    let length = gather(Length::new(length?, units.length).map_err(placed("length")), out);
    let angle = gather(Angle::new(angle?, units.angle).map_err(placed("angle")), out);
    gather(Tolerance::new(length?, angle?).map_err(|report| report.with_input(at.input())), out)
}

/// The request's steps, each checked, with every bad input in `out`.
fn steps(root: &At, fields: &Map<String, Value>, units: Units, out: &mut Vec<Report>) -> Option<Vec<Step>> {
    let at = root.field("steps");
    let given = gather(needed(root, fields, "steps"), out)?;
    let Value::Array(items) = given else {
        out.push(wrong_type(&at, given, "list"));
        return None;
    };
    let mut labels: BTreeMap<String, u32> = BTreeMap::new();
    let mut steps = Vec::new();
    for (place, item) in items.iter().enumerate() {
        let index = u32::try_from(place).unwrap_or(u32::MAX);
        steps.extend(step(&at.item(place), (index, units), item, &mut labels, out));
    }
    Some(steps)
}

/// The step at `at`, the request's `index`th, whose label must not be one an earlier step in `labels` has.
fn step(
    at: &At,
    (index, units): (u32, Units),
    item: &Value,
    labels: &mut BTreeMap<String, u32>,
    out: &mut Vec<Report>,
) -> Option<Step> {
    let Value::Object(fields) = item else {
        out.push(wrong_type(&at.clone().in_step(index, None), item, "object"));
        return None;
    };
    let at = at.clone().in_step(index, fields.get("op").and_then(Value::as_str));
    let label = fields.get("label").and_then(|given| gather(label(&at.field("label"), given, labels, index), out));
    let op = needed(&at, fields, "op").and_then(|given| text(&at.field("op"), given));
    let operation = gather(op.and_then(|name| Operation::asked(&at.field("op"), name)), out)?;
    let mut parameters = fields.clone();
    parameters.remove("op");
    parameters.remove("label");
    let args = args::read(&at, operation, &parameters, units, out);
    Some(Step { index, label, operation, parameters, args })
}

/// The label at `at`, of the `index`th step, which no earlier step in `labels` may have; it is added there.
fn label(at: &At, given: &Value, labels: &mut BTreeMap<String, u32>, index: u32) -> Result<String, Report> {
    let label = text(at, given)?;
    if label.is_empty() || EntityRef::parse(label).is_some() {
        return Err(at.report(Code::InputLabelNotValid).with_input(at.input().given(label)));
    }
    if let Some(&first) = labels.get(label) {
        let report = at.report(Code::InputDuplicateLabel).with_input(at.input().given(label));
        return Err(report.with_value(NamedValue::count("first", first)));
    }
    labels.insert(label.to_string(), index);
    Ok(label.to_string())
}

#[cfg(test)]
mod tests;
