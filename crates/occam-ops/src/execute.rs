//! Source: original
//! What each operation does to a model, given its step and the labels of the steps before it. Each operation's step adds
//! its arm here as it lands; until then a step that reached this would be refused as `request.not-supported`, though a
//! request asking for one is refused when it is read. A step built by hand without what its operation reads is refused
//! as missing it.

use occam_report::{Code, EntityRef, Fault, Input, Report};
use occam_topo::Edit;

use crate::args::Args;
use crate::construct;
use crate::operand::Labels;
use crate::operation::Operation;
use crate::primitive;
use crate::request::Step;
use crate::sweep;

/// Does `step` in `edit`, with `labels` naming what earlier steps made, and gives what it made, if it made one thing a
/// later step can name by its label.
///
/// # Errors
///
/// What the operation reports. [`Code::NotSupported`], fault not implemented, for an operation this build does not do.
pub fn execute(edit: &mut Edit, step: &Step, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let place = |field: &str| {
        Input::at(&format!("steps[{}]{field}", step.index)).in_step(step.index, Some(step.operation.name()))
    };
    match (step.operation, &step.args) {
        (Operation::Wire, Args::Wire(args)) => construct::wire(edit, args),
        (Operation::Face, Args::Face(args)) => construct::face(edit, args, labels),
        (
            Operation::Box
            | Operation::Cylinder
            | Operation::Sphere
            | Operation::Cone
            | Operation::Torus
            | Operation::Wedge,
            Args::Primitive(args),
        ) => primitive::make(edit, args),
        (Operation::Extrude, Args::Extrude(args)) => sweep::extrude(edit, args, labels),
        (Operation::Revolve, Args::Revolve(args)) => sweep::revolve(edit, args, labels),
        (operation, _) if operation.supported() => {
            Err(Report::new(Code::InputMissing, Fault::InvalidInput).with_input(place("")))
        }
        (_, _) => Err(Report::new(Code::NotSupported, Fault::NotImplemented).with_input(place(".op"))),
    }
}

#[cfg(test)]
mod tests {
    use occam_topo::Snapshot;
    use serde_json::Map;

    use super::*;
    use crate::request::default_tolerance;

    #[test]
    fn a_step_built_by_hand_without_what_its_operation_reads_is_refused_as_missing_it() {
        let snapshot = Snapshot::empty(default_tolerance().unwrap());
        for operation in Operation::ALL.into_iter().filter(|op| op.supported()) {
            let step = Step { index: 2, label: None, operation, parameters: Map::new(), args: Args::None };
            let report = execute(&mut snapshot.edit(), &step, &Labels::default()).unwrap_err();
            assert_eq!((report.code, report.fault), (Code::InputMissing, Some(Fault::InvalidInput)));
            assert_eq!(report.input, Some(Input::at("steps[2]").in_step(2, Some(operation.name()))));
        }
    }

    #[test]
    fn an_operation_this_build_does_not_do_is_refused_if_a_step_reaches_it() {
        let snapshot = Snapshot::empty(default_tolerance().unwrap());
        for operation in Operation::ALL.into_iter().filter(|op| !op.supported()) {
            let step = Step { index: 3, label: None, operation, parameters: Map::new(), args: Args::None };
            let report = execute(&mut snapshot.edit(), &step, &Labels::default()).unwrap_err();
            assert_eq!((report.code, report.fault), (Code::NotSupported, Some(Fault::NotImplemented)));
            let input = Input::at("steps[3].op").in_step(3, Some(operation.name()));
            assert_eq!(report.input, Some(input), "{operation:?}");
        }
    }
}
