//! Source: original
//! The checker (PARITY 1.10). [`entity`] holds what one entity's record and its neighbours' must satisfy, and [`body`]
//! what a whole body must, shell by shell, against the faces that name each shell, which [`members`] finds. `Edit::commit`
//! runs [`edit`], which checks what the edit touched and its neighbours (P1): [`entity`] over that scope, then the
//! answers [`body`] would give, reached from what changed rather than by walking the model. Each failure is one `check.invariant-failed`
//! report naming its entities, with its reason from the closed list.

mod incremental;
mod local;
pub(crate) mod read;
mod scope;
mod shell;

use occam_report::{Code, EntityRef, Fault, Reason, Report};
pub(crate) use shell::Members;

use crate::edit::Edit;
use crate::kind::{Body, Id, Kind};
use crate::snapshot::{Holds, Model};
use read::Walk;

/// The report of one failure.
fn failed(reason: Reason, entities: &[EntityRef]) -> Report {
    let report = Report::new(Code::InvariantFailed, Fault::Internal).with_reason(reason);
    entities.iter().fold(report, |report, &entity| report.with_entity(entity))
}

/// The entry a walk that should have ended, or come round, went wrong at.
fn culprit<T: Copy>(walk: &Walk<T>) -> Option<T> {
    match walk {
        Walk::Ring(items) | Walk::End(items) => items.last().copied(),
        Walk::Broken(_, at) => Some(*at),
    }
}

/// Checks a list `owner` holds: that it ends, or comes round if `ring`, failing with `broken` naming where it went wrong,
/// and that each entry `named` it. Gives the entries if the list is sound.
fn holds<K: Kind>(
    out: &mut Vec<Report>,
    owner: EntityRef,
    walk: &Walk<Id<K>>,
    ring: bool,
    broken: Reason,
    named: impl Fn(Id<K>) -> bool,
) -> Option<Vec<Id<K>>> {
    let Some(items) = walk.sound(ring) else {
        let at = culprit(walk).map(Id::entity);
        out.push(failed(broken, &[owner].into_iter().chain(at).collect::<Vec<_>>()));
        return None;
    };
    for &item in items.iter().filter(|&&item| !named(item)) {
        out.push(failed(Reason::ListedElsewhere, &[owner, item.entity()]));
    }
    Some(items.to_vec())
}

/// Every invariant of one entity that reads its record and its neighbours' records, then those that read their
/// geometry, in a fixed order.
pub(crate) fn entity(model: &Model, entity: EntityRef, out: &mut Vec<Report>) {
    local::check(model, entity, out);
    crate::geometric::entity(model, entity, out);
}

/// The faces that name each shell, for [`body`]: one scan of every face.
pub(crate) fn members(model: &Model) -> Members {
    Members::of(model)
}

/// Every invariant of one body as a whole: each face that names one of its shells joined through their edges to the face
/// the shell begins from, and, for a solid, a whole genus.
pub(crate) fn body(model: &Model, members: &Members, body: Id<Body>, out: &mut Vec<Report>) {
    shell::body(model, members, body, out);
}

/// Every failure of `edit`, in a fixed order: full tables, reserved ids never written, then the invariants of what it
/// touched and its neighbours, then the genus and the joining of the shells it changed.
pub(crate) fn edit(edit: &Edit) -> Vec<Report> {
    let mut found: Vec<Report> = edit.full.iter().map(|_| failed(Reason::TableFull, &[])).collect();
    let unwritten = edit.created.iter().filter(|&&entity| edit.next.holds(entity) == Holds::Reserved);
    found.extend(unwritten.map(|&entity| failed(Reason::ReservedNotSet, &[entity])));
    for touched in scope::scope(edit) {
        entity(&edit.next, touched, &mut found);
    }
    let touched = incremental::Touched::of(edit);
    incremental::genus(&touched, &mut found);
    incremental::joined(&touched, &mut found);
    found
}

#[cfg(test)]
mod membership;
#[cfg(test)]
mod tests;
