//! Source: original
//! The request runner (docs/architecture/operations.md). A request is read and checked whole before anything is done;
//! then its steps run in order, each through an edit that `commit` checks, on one thread. The first step that fails
//! stops the run, and the model is the one the last good step left, so nothing that failed changes it (L5). Each step
//! that ran says what it created, changed and deleted, and where each new entity came from (the brief). As the result
//! leaves, every id it names is described, in one batch (O26).

mod wire;

use occam_math::Exec;
use occam_ops::{Labels, Measured, Operation, Request, Step, default_tolerance};
use occam_report::{
    Code, Description, DisplayUnits, EntityRef, Fault, Input, NamedValue, Quantity, Remedy, RemedyKind, Report,
};
use occam_topo::{Changes, Edit, Snapshot};
use occam_units::Tolerance;

use crate::describe::{self, Source, Steps};

pub use wire::ran_json;

/// Where a model's tolerance came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToleranceFrom {
    /// The request gave it, for a new model.
    Request,
    /// The request gave none, for a new model: the default of O12.
    Default,
    /// The model given was built to it, and it governs.
    Model,
}

/// A step that ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    /// Its index in the request.
    pub step: u32,
    /// Its label, if it has one.
    pub label: Option<String>,
    /// What it asked for.
    pub operation: Operation,
    /// What it made that a later step can name by its label.
    pub made: Option<EntityRef>,
    /// What it created, changed and deleted, and where each new entity came from.
    pub changes: Changes,
    /// What each entity it names is: as the run left it, or as it last was if it is gone (O26).
    pub described: Vec<Description>,
    /// What it measured, if it was a measure, which changes nothing.
    pub measured: Option<Measured>,
}

impl Done {
    /// Every entity it names: what it made, created, changed and deleted, both ends of each lineage, and what it
    /// measured.
    pub(crate) fn named(&self) -> impl Iterator<Item = EntityRef> + '_ {
        let changes = &self.changes;
        let lineage = changes.lineage.iter().flat_map(|l| [l.new, l.old]);
        let listed = changes.created.iter().chain(&changes.changed).chain(&changes.deleted).copied();
        let measured = self.measured.as_ref().map(|m| m.target);
        self.made.into_iter().chain(measured).chain(listed).chain(lineage)
    }
}

/// Why a run stopped short.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    /// The step that failed, or none if the request was refused before any step ran.
    pub step: Option<u32>,
    /// What went wrong, each report with what the entities it names are.
    pub reports: Vec<Report>,
}

/// What running a request did.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    /// The model after the last step that succeeded: the one given if none did, and none if no model was given and the
    /// request was refused before one was made.
    pub model: Option<Snapshot>,
    /// The units the request's numbers were in, which its wording uses.
    pub units: DisplayUnits,
    /// The model's tolerance and where it came from, once there is a model.
    pub tolerance: Option<(Tolerance, ToleranceFrom)>,
    /// The steps that ran, in order.
    pub done: Vec<Done>,
    /// Why it stopped short, if it did.
    pub failed: Option<Failed>,
}

/// Runs the request written `request` on `model`, or on a new model if there is none.
#[must_use]
pub fn run(model: Option<&Snapshot>, request: &str) -> Ran {
    match Request::parse(request) {
        Ok(request) => run_request(model, &request, occam_ops::execute),
        Err(reports) => refused(model, DisplayUnits::default(), reports),
    }
}

/// What a request refused before any step ran did: nothing.
fn refused(model: Option<&Snapshot>, units: DisplayUnits, reports: Vec<Report>) -> Ran {
    let sources: Vec<Source<'_>> = model.into_iter().map(Source::Model).collect();
    let reports = reports.into_iter().map(|r| describe::report(r, &sources, &Steps::new())).collect();
    let failed = Some(Failed { step: None, reports });
    Ran { model: model.cloned(), units, tolerance: None, done: Vec::new(), failed }
}

/// What a step that ran gives: the model it left, what it made, what it changed, and what it measured.
type Stepped = (Snapshot, Option<EntityRef>, Changes, Option<Measured>);

/// What a step that failed gives: its reports, and its edit as it stood when it was refused.
type Stopped = (Vec<Report>, Box<Edit>);

/// A step that failed: its index, its reports, and its edit as it stood when it was refused.
pub(crate) type Refused = (u32, Vec<Report>, Box<Edit>);

/// Runs `request`, read and checked, with `execute` doing each step.
pub(crate) fn run_request(
    model: Option<&Snapshot>,
    request: &Request,
    mut execute: impl FnMut(&mut Edit, &Step, &Labels) -> Result<Option<EntityRef>, Report>,
) -> Ran {
    let units = DisplayUnits { length: request.units.length, angle: request.units.angle };
    let (first, tolerance) = match start(model, request) {
        Ok(started) => started,
        Err(report) => return refused(model, units, vec![report]),
    };
    // Each model the run passes through, kept to describe what was gone by the end; a snapshot shares its chunks.
    let (mut models, mut steps, mut labels, mut done) =
        (vec![first.clone()], Steps::new(), Labels::default(), Vec::new());
    let (mut current, mut stopped) = (first, None);
    for step in &request.steps {
        steps.insert(current.step().saturating_add(1), (step.index, step.operation));
        match one(&current, step, &labels, &mut execute) {
            Ok((next, made, changes, measured)) => {
                if let (Some(label), Some(made)) = (&step.label, made) {
                    labels.insert(label, made);
                }
                let label = step.label.clone();
                let operation = step.operation;
                let described = Vec::new();
                done.push(Done { step: step.index, label, operation, made, changes, described, measured });
                models.push(next.clone());
                current = next;
            }
            Err((reports, edit)) => {
                stopped = Some((step.index, reports, edit));
                break;
            }
        }
    }
    let failed = describe::describe_all(&models, &steps, &mut done, stopped);
    Ran { model: Some(current), units, tolerance: Some(tolerance), done, failed }
}

/// Does `step` on `model` and commits it, or, for a measure, which changes nothing, measures `model`; if it fails, gives
/// its reports and its edit as it stood.
fn one(
    model: &Snapshot,
    step: &Step,
    labels: &Labels,
    execute: &mut impl FnMut(&mut Edit, &Step, &Labels) -> Result<Option<EntityRef>, Report>,
) -> Result<Stepped, Stopped> {
    if step.operation == Operation::Measure {
        return match occam_ops::measure(model, step, labels) {
            Ok(measured) => Ok((model.clone(), None, Changes::default(), Some(measured))),
            Err(report) => Err((vec![report], Box::new(model.edit()))),
        };
    }
    let mut edit = model.edit();
    let made = match execute(&mut edit, step, labels) {
        Ok(made) => made,
        Err(report) => return Err((vec![report], Box::new(edit))),
    };
    let (next, changes) = edit.try_commit(&Exec::serial())?;
    Ok((next, made, changes, None))
}

/// The model the run starts from, and its tolerance: `model`'s own, or a new model's, the request's or the default.
fn start(model: Option<&Snapshot>, request: &Request) -> Result<(Snapshot, (Tolerance, ToleranceFrom)), Report> {
    match (model, request.tolerance) {
        (None, Some(asked)) => Ok((Snapshot::empty(asked), (asked, ToleranceFrom::Request))),
        (None, None) => {
            let default = default_tolerance()?;
            Ok((Snapshot::empty(default), (default, ToleranceFrom::Default)))
        }
        (Some(model), asked) => {
            let own = model.tolerance();
            if let Some(asked) = asked.filter(|asked| asked.length() < own.length() || asked.angle() < own.angle()) {
                return Err(too_fine(asked, own));
            }
            Ok((model.clone(), (own, ToleranceFrom::Model)))
        }
    }
}

/// A tolerance asked of a model built to a coarser one.
fn too_fine(asked: Tolerance, own: Tolerance) -> Report {
    let remedy = Remedy::new(RemedyKind::UseModelTolerance)
        .with_value(NamedValue::length("tolerance", own.length().millimetres()))
        .with_value(NamedValue::new("angle", Quantity::Angle, own.angle().radians()));
    Report::new(Code::ToleranceBelowModel, Fault::InvalidInput)
        .with_input(Input::at("tolerance"))
        .with_value(NamedValue::length("requested", asked.length().millimetres()))
        .with_value(NamedValue::length("model", own.length().millimetres()))
        .with_value(NamedValue::new("requested-angle", Quantity::Angle, asked.angle().radians()))
        .with_value(NamedValue::new("model-angle", Quantity::Angle, own.angle().radians()))
        .with_remedy(remedy)
}

#[cfg(test)]
mod tests;
