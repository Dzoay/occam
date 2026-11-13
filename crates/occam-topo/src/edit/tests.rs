//! Source: original
//! The edit's tests: what it lists as changed, what `commit` refuses, and what it carries along lineage.

use occam_report::{Code, Fault, Reason};
use occam_units::Length;

use super::*;
use crate::kind::{CurveGeom, Edge, Face, Vertex};
use crate::models::join_star;
use crate::records::{Around, EdgeRecord, EdgeRole, End, VertexRecord};
use crate::testing::{number, record, serial, tolerance, vertex, x_axis};

/// The reports' reasons and entities, to compare at once.
fn failures(reports: &[Report]) -> Vec<(Reason, Vec<EntityRef>)> {
    for report in reports {
        assert_eq!((report.code, report.fault), (Code::InvariantFailed, Some(Fault::Internal)));
    }
    reports.iter().map(|r| (r.reason.unwrap(), r.entities.clone())).collect()
}

/// An edge from `a` to `b` along `curve`, made in `role`, joined into the star of each end's vertex that is live.
fn edge(edit: &mut Edit, curve: Id<CurveGeom>, (a, b): (Id<Vertex>, Id<Vertex>), role: StepRole) -> Id<Edge> {
    let id = edit.reserve::<Edge>();
    let around = (
        join_star(edit, a, Around { edge: id, end: End::Start }),
        join_star(edit, b, Around { edge: id, end: End::Finish }),
    );
    let record = EdgeRecord {
        curve,
        range: (0.0, 1.0),
        ends: (a, b),
        around,
        first_use: None,
        role: EdgeRole::Ordinary,
        deviation: Length::from_millimetres(0.0),
        born: edit.provenance(role),
    };
    edit.set(id, record).unwrap();
    id
}

/// A model of one edge between two vertices, each with the edge in its star, made at step 1 as a profile.
fn one_edge() -> (Snapshot, Id<Edge>, [Id<Vertex>; 2]) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let curve = edit.add(x_axis());
    let (a, b) = (edit.add(vertex(0.0)), edit.add(vertex(1.0)));
    let e = edge(&mut edit, curve, (a, b), StepRole::Profile);
    let (snapshot, changes) = edit.commit(&serial()).unwrap();
    assert_eq!(changes.created.len(), 4);
    (snapshot, e, [a, b])
}

/// The record `id` has in `edit`.
fn record_in<K: crate::kind::Kind>(edit: &Edit, id: Id<K>) -> K::Record {
    edit.record(id).unwrap()
}

/// A new vertex with no edges yet.
fn edit_vertex(edit: &mut Edit) -> Id<Vertex> {
    edit.add(vertex(7.0))
}

#[test]
fn changes_name_each_entity_once_in_the_list_it_belongs_to() {
    let mut first = Snapshot::empty(tolerance()).edit();
    let v: Vec<Id<Vertex>> = (0..3).map(|k| first.add(vertex(f64::from(k)))).collect();
    let (base, _) = first.commit(&serial()).unwrap();
    let mut edit = base.edit();
    edit.remove(v[1]).unwrap();
    let reused = edit.add(vertex(10.0));
    edit.set(v[0], vertex(20.0)).unwrap();
    edit.set(v[0], vertex(21.0)).unwrap();
    let made = edit.add(vertex(30.0));
    edit.set(made, vertex(31.0)).unwrap();
    let passing = edit.add(vertex(40.0));
    edit.remove(passing).unwrap();
    edit.set(v[2], vertex(50.0)).unwrap();
    edit.remove(v[2]).unwrap();
    assert_eq!(edit.record(made), Some(vertex(31.0)));
    let (after, changes) = edit.commit(&serial()).unwrap();
    let names = |list: &[EntityRef]| list.iter().map(|e| e.text()).collect::<Vec<_>>();
    assert_eq!(names(&changes.created), ["vertex:1/2", "vertex:3/1"], "a freed slot reused, and created then changed");
    assert_eq!(names(&changes.changed), ["vertex:0/1"], "changed twice, listed once");
    assert_eq!(names(&changes.deleted), ["vertex:1/1", "vertex:2/1"], "changed then removed is only deleted");
    assert_eq!((reused.entity().text(), made.entity().text()), ("vertex:1/2".into(), "vertex:3/1".into()));
    assert_eq!(after.ids::<Vertex>(), [v[0], reused, made]);
    assert_eq!(record(&after, v[0]), Some(&vertex(21.0)));
    assert_eq!(base.ids::<Vertex>(), v, "the snapshot it began from is unchanged");
    assert_eq!((base.step(), after.step()), (1, 2));
}

#[test]
fn a_reserved_id_never_set_fails_commit_is_named_and_leaves_the_input_as_it_was() {
    let (base, e, [a, _]) = one_edge();
    let before = base.clone();
    let mut edit = base.edit();
    let unset = edit.reserve::<Face>();
    let wider = Length::from_millimetres(0.5);
    edit.set(a, VertexRecord { radius: wider, ..record(&base, a).unwrap().clone() }).unwrap();
    let reports = edit.commit(&serial()).unwrap_err();
    assert_eq!(failures(&reports), [(Reason::ReservedNotSet, vec![unset.entity()])]);
    assert_eq!(base, before);
    assert_eq!(record(&base, a).map(|v| v.radius), Some(Length::from_millimetres(0.0)));
    assert_eq!(base.resolve::<Edge>(e.entity()), Ok(e));
}

#[test]
fn a_reserved_id_removed_before_it_is_set_appears_nowhere() {
    let (base, _, _) = one_edge();
    let mut edit = base.edit();
    let dropped = edit.reserve::<Vertex>();
    edit.remove(dropped).unwrap();
    assert_eq!(edit.remove(dropped).map_err(|r| r.code), Err(Code::StaleId), "already gone");
    let (after, changes) = edit.commit(&serial()).unwrap();
    assert_eq!(changes, Changes::default());
    assert_eq!(after.ids::<Vertex>(), base.ids::<Vertex>());
    let again = after.edit().reserve::<Vertex>();
    assert_eq!((again.slot(), again.generation().get()), (dropped.slot(), 2), "its slot was freed");
}

#[test]
fn a_record_linking_to_an_entity_not_live_fails_commit_naming_both() {
    let (base, e, [_, b]) = one_edge();
    let mut edit = base.edit();
    let curve = record(&base, e).unwrap().curve;
    let lone = edit.add(vertex(3.0));
    edit.remove(lone).unwrap();
    let to_edge = edit.add(vertex(0.0));
    let dangling = edge(&mut edit, curve, (to_edge, lone), StepRole::Made);
    assert_eq!(record_in(&edit, to_edge).star, Some(Around { edge: dangling, end: End::Start }));
    let reports = edit.commit(&serial()).unwrap_err();
    assert_eq!(failures(&reports), [(Reason::DeadLink, vec![dangling.entity(), lone.entity()])]);
    assert!(base.resolve::<Vertex>(to_edge.entity()).is_err() && base.resolve::<Vertex>(b.entity()).is_ok());
}

#[test]
fn a_record_left_linking_to_one_the_edit_removed_fails_commit() {
    let (base, e, [a, b]) = one_edge();
    let mut edit = base.edit();
    edit.remove(b).unwrap();
    let reports = edit.commit(&serial()).unwrap_err();
    assert_eq!(failures(&reports), [(Reason::DeadLink, vec![e.entity(), b.entity()])]);
    let mut whole = base.edit();
    for gone in [a, b] {
        whole.remove(gone).unwrap();
    }
    whole.remove(e).unwrap();
    let (empty, changes) = whole.commit(&serial()).unwrap();
    assert_eq!((empty.ids::<Edge>(), changes.deleted.len()), (Vec::new(), 3), "removing all three is clean");
    let mut relinked = base.edit();
    relinked.remove(b).unwrap();
    let c = relinked.add(VertexRecord { star: Some(Around { edge: e, end: End::Finish }), ..vertex(1.0) });
    let moved = EdgeRecord { ends: (a, c), ..record(&base, e).unwrap().clone() };
    relinked.set(e, moved).unwrap();
    assert_eq!(relinked.commit(&serial()).map(|(_, c)| c.changed), Ok(vec![e.entity()]), "relinked first");
}

#[test]
fn derive_records_lineage_and_carries_the_first_source_of_its_own_kinds_provenance() {
    let (base, e, [a, b]) = one_edge();
    let mut edit = base.edit();
    let curve = record(&base, e).unwrap().curve;
    let split = edge(&mut edit, curve, (a, b), StepRole::Made);
    let copied = edge(&mut edit, curve, (a, b), StepRole::Made);
    let far = (edit_vertex(&mut edit), edit_vertex(&mut edit));
    let gone = edge(&mut edit, curve, far, StepRole::Made);
    let gone_ends = record_in(&edit, gone).ends;
    edit.derive(split.entity(), a.entity(), Derivation::Copied).unwrap();
    edit.derive(split.entity(), e.entity(), Derivation::Split).unwrap();
    edit.derive(copied.entity(), b.entity(), Derivation::Copied).unwrap();
    edit.derive(gone.entity(), e.entity(), Derivation::Split).unwrap();
    edit.remove(gone).unwrap();
    edit.remove(gone_ends.0).unwrap();
    edit.remove(gone_ends.1).unwrap();
    let late = edit.add(vertex(4.0));
    assert_eq!(edit.derive(split.entity(), late.entity(), Derivation::Merged).map_err(|r| r.code), Err(Code::StaleId));
    let refused = edit.derive(gone.entity(), e.entity(), Derivation::Split).unwrap_err();
    assert_eq!((refused.code, number(&refused, "live")), (Code::StaleId, Some(0.0)));
    edit.remove(late).unwrap();
    let (after, changes) = edit.commit(&serial()).unwrap();
    let lineage = |new: Id<Edge>, old: EntityRef, how| Lineage { new: new.entity(), old, how };
    assert_eq!(
        changes.lineage,
        [
            lineage(split, a.entity(), Derivation::Copied),
            lineage(split, e.entity(), Derivation::Split),
            lineage(copied, b.entity(), Derivation::Copied),
        ]
    );
    let born = |id: Id<Edge>| record(&after, id).map(|r| r.born);
    assert_eq!(born(split), Some(Provenance { step: 1, role: StepRole::Profile }), "from the edge it split");
    assert_eq!(born(copied), Some(Provenance { step: 2, role: StepRole::Made }), "a vertex is not its kind");
}

#[test]
fn of_two_sources_of_its_own_kind_an_entity_takes_the_first_ones_provenance() {
    // The edge made at step 1 as a profile, and another at step 2 as a tool: one merged from both, the first named
    // first, keeps the first's.
    let (base, e, [a, b]) = one_edge();
    let curve = record(&base, e).unwrap().curve;
    let mut edit = base.edit();
    let tool = edge(&mut edit, curve, (a, b), StepRole::Tool);
    let (base, _) = edit.commit(&serial()).unwrap();
    let mut edit = base.edit();
    let merged = edge(&mut edit, curve, (a, b), StepRole::Made);
    edit.derive(merged.entity(), e.entity(), Derivation::Merged).unwrap();
    edit.derive(merged.entity(), tool.entity(), Derivation::Merged).unwrap();
    let (after, _) = edit.commit(&serial()).unwrap();
    assert_eq!(record(&after, merged).map(|r| r.born), Some(Provenance { step: 1, role: StepRole::Profile }));
}

#[test]
fn the_step_counts_over_the_model_and_a_dropped_edit_changes_nothing() {
    let empty = Snapshot::empty(tolerance());
    let mut edit = empty.edit();
    assert_eq!((edit.step(), edit.provenance(StepRole::Tool)), (1, Provenance { step: 1, role: StepRole::Tool }));
    edit.add(vertex(1.0));
    assert_eq!(edit.base(), &empty);
    drop(edit);
    assert_eq!(empty.ids::<Vertex>(), Vec::new());
    let (one, _) = empty.edit().commit(&serial()).unwrap();
    let (two, _) = one.edit().commit(&serial()).unwrap();
    assert_eq!((one.step(), two.step(), two.edit().step()), (1, 2, 3));
}

#[test]
fn writing_or_removing_a_stale_id_is_refused_and_says_what_its_slot_holds() {
    let (base, _, [a, _]) = one_edge();
    let mut edit = base.edit();
    edit.remove(a).unwrap();
    let refused = edit.set(a, vertex(1.0)).unwrap_err();
    assert_eq!(
        (refused.code, refused.entities.clone(), number(&refused, "live")),
        (Code::StaleId, vec![a.entity()], Some(0.0))
    );
    let newer = edit.add(vertex(2.0));
    assert_eq!(newer.slot(), a.slot());
    assert_eq!(edit.remove(a).map_err(|r| number(&r, "live")), Err(Some(2.0)));
    assert_eq!(edit.record(a), None);
}

#[test]
fn a_table_with_no_slot_left_fails_the_commit() {
    let base = Snapshot { model: Model::empty(tolerance(), 1) };
    let mut edit = base.edit();
    let first = edit.add(vertex(1.0));
    let second = edit.add(vertex(2.0));
    assert_eq!((first.slot(), second.slot()), (0, u32::MAX));
    assert_eq!(edit.set(second, vertex(3.0)).map_err(|r| r.code), Err(Code::StaleId));
    edit.add(x_axis());
    let reports = edit.commit(&serial()).unwrap_err();
    assert_eq!(failures(&reports), [(Reason::TableFull, vec![])]);
}
