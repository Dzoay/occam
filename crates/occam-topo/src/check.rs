//! Source: original
//! What `commit` checks before it makes a snapshot, on what the edit touched: that each reserved id was written, that
//! no record touched links to an entity that is not live, that no record left alone links to one the edit removed, and
//! that no table ran out of slots. Each failure is one report naming its entities.

use occam_report::{Code, EntityRef, Fault, Reason, Report};

use crate::edit::Edit;
use crate::snapshot::Holds;

/// The report of one failure.
fn failed(reason: Reason, entities: &[EntityRef]) -> Report {
    let report = Report::new(Code::InvariantFailed, Fault::Internal).with_reason(reason);
    entities.iter().fold(report, |report, &entity| report.with_entity(entity))
}

/// Every failure of `edit`, in a fixed order: full tables, then reserved ids, then links.
pub(crate) fn edit(edit: &Edit) -> Vec<Report> {
    let mut found: Vec<Report> = edit.full.iter().map(|_| failed(Reason::TableFull, &[])).collect();
    let unwritten = edit.created.iter().filter(|&&entity| edit.next.holds(entity) == Holds::Reserved);
    found.extend(unwritten.map(|&entity| failed(Reason::ReservedNotSet, &[entity])));
    dead_links(edit, &mut found);
    left_linking(edit, &mut found);
    found
}

/// What `entity`'s record links to in `model`, each once, in id order.
fn links_of(model: &crate::snapshot::Model, entity: EntityRef) -> Vec<EntityRef> {
    let mut links = Vec::new();
    model.links(entity, &mut links);
    links.sort_unstable();
    links.dedup();
    links
}

/// Each link of a created or changed record to an entity that is not live.
fn dead_links(edit: &Edit, found: &mut Vec<Report>) {
    for &entity in edit.created.iter().chain(&edit.changed) {
        for link in links_of(&edit.next, entity) {
            if !edit.next.is_live(link) {
                found.push(failed(Reason::DeadLink, &[entity, link]));
            }
        }
    }
}

/// Each record the edit left alone that links to an entity it removed, found from the removed entity's own links.
fn left_linking(edit: &Edit, found: &mut Vec<Report>) {
    let touched = |entity: &EntityRef| edit.created.contains(entity) || edit.changed.contains(entity);
    for &gone in &edit.deleted {
        for neighbour in links_of(&edit.base.model, gone) {
            if !touched(&neighbour) && links_of(&edit.next, neighbour).contains(&gone) {
                found.push(failed(Reason::DeadLink, &[neighbour, gone]));
            }
        }
    }
}
