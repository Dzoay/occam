//! Source: original
//! What each operation does to a model, given its step and the labels of the steps before it. Each operation's step adds
//! its arm here as it lands; until then a step that reached this would be refused as `request.not-supported`, though a
//! request asking for one is refused when it is read.

use occam_report::{Code, EntityRef, Fault, Input, Report};
use occam_topo::Edit;

use crate::operand::Labels;
use crate::operation::Operation;
use crate::request::Step;

/// Does `step` in `edit`, with `labels` naming what earlier steps made, and gives what it made, if it made one thing a
/// later step can name by its label.
///
/// # Errors
///
/// What the operation reports. [`Code::NotSupported`], fault not implemented, for an operation this build does not do.
pub fn execute(edit: &mut Edit, step: &Step, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let (_, _) = (edit, labels);
    match step.operation {
        Operation::Box
        | Operation::Cylinder
        | Operation::Sphere
        | Operation::Cone
        | Operation::Torus
        | Operation::Wedge
        | Operation::Wire
        | Operation::Face
        | Operation::Extrude
        | Operation::Revolve
        | Operation::Transform
        | Operation::Copy
        | Operation::Measure
        | Operation::Boolean
        | Operation::Fillet
        | Operation::Chamfer => {
            let input =
                Input::at(&format!("steps[{}].op", step.index)).in_step(step.index, Some(step.operation.name()));
            Err(Report::new(Code::NotSupported, Fault::NotImplemented).with_input(input))
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_topo::Snapshot;
    use serde_json::Map;

    use super::*;
    use crate::request::default_tolerance;

    #[test]
    fn an_operation_this_build_does_not_do_is_refused_if_a_step_reaches_it() {
        let snapshot = Snapshot::empty(default_tolerance().unwrap());
        for operation in Operation::ALL.into_iter().filter(|op| !op.supported()) {
            let step = Step { index: 3, label: None, operation, parameters: Map::new() };
            let report = execute(&mut snapshot.edit(), &step, &Labels::default()).unwrap_err();
            assert_eq!((report.code, report.fault), (Code::NotSupported, Some(Fault::NotImplemented)));
            let input = Input::at("steps[3].op").in_step(3, Some(operation.name()));
            assert_eq!(report.input, Some(input), "{operation:?}");
        }
    }
}
