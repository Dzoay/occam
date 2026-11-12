//! Source: original
//! The request runner (docs/architecture/operations.md). A request is read and checked whole before anything is done;
//! then its steps run in order, each through an edit that `commit` checks, on one thread. The first step that fails
//! stops the run, and the model is the one the last good step left, so nothing that failed changes it (L5). Each step
//! that ran says what it created, changed and deleted, and where each new entity came from (the brief).

mod wire;

use occam_math::Exec;
use occam_ops::{Labels, Operation, Request, Step, default_tolerance};
use occam_report::{Code, DisplayUnits, EntityRef, Fault, Input, NamedValue, Quantity, Remedy, RemedyKind, Report};
use occam_topo::{Changes, Edit, Snapshot};
use occam_units::Tolerance;

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
}

/// Why a run stopped short.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    /// The step that failed, or none if the request was refused before any step ran.
    pub step: Option<u32>,
    /// What went wrong.
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
    let failed = Some(Failed { step: None, reports });
    Ran { model: model.cloned(), units, tolerance: None, done: Vec::new(), failed }
}

/// Runs `request`, read and checked, with `execute` doing each step.
pub(crate) fn run_request(
    model: Option<&Snapshot>,
    request: &Request,
    mut execute: impl FnMut(&mut Edit, &Step, &Labels) -> Result<Option<EntityRef>, Report>,
) -> Ran {
    let units = DisplayUnits { length: request.units.length, angle: request.units.angle };
    let (mut current, tolerance) = match start(model, request) {
        Ok(started) => started,
        Err(report) => return refused(model, units, vec![report]),
    };
    let (mut labels, mut done) = (Labels::default(), Vec::new());
    for step in &request.steps {
        match one(&current, step, &labels, &mut execute) {
            Ok((next, made, changes)) => {
                if let (Some(label), Some(made)) = (&step.label, made) {
                    labels.insert(label, made);
                }
                let label = step.label.clone();
                done.push(Done { step: step.index, label, operation: step.operation, made, changes });
                current = next;
            }
            Err(reports) => {
                let failed = Some(Failed { step: Some(step.index), reports });
                return Ran { model: Some(current), units, tolerance: Some(tolerance), done, failed };
            }
        }
    }
    Ran { model: Some(current), units, tolerance: Some(tolerance), done, failed: None }
}

/// Does `step` on `model` and commits it.
fn one(
    model: &Snapshot,
    step: &Step,
    labels: &Labels,
    execute: &mut impl FnMut(&mut Edit, &Step, &Labels) -> Result<Option<EntityRef>, Report>,
) -> Result<(Snapshot, Option<EntityRef>, Changes), Vec<Report>> {
    let mut edit = model.edit();
    let made = execute(&mut edit, step, labels).map_err(|report| vec![report])?;
    let (next, changes) = edit.commit(&Exec::serial())?;
    Ok((next, made, changes))
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
