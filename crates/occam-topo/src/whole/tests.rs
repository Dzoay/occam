//! Source: original
//! The whole-model check's tests: nothing on a clean model, and every failure once, in one order at any thread count.

use occam_report::{Code, Reason};

use super::*;
use crate::kind::sealed::Sealed;
use crate::kind::{CurveGeom, Edge};
use crate::models::{Built, build, cube, cylinder, frame, sheet, wire};
use crate::table::CHUNK;
use crate::testing::{serial, tolerance};

/// The cube, the frame, the cylinder, the sheet and the wire in one snapshot.
fn everything() -> (Snapshot, Vec<Built>) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = [cube(), frame(), cylinder(), sheet(), wire()].iter().map(|spec| build(&mut edit, spec)).collect();
    (edit.commit(&serial()).unwrap().0, built)
}

/// Frees `id` straight from its table, as no edit could, leaving whatever links to it.
fn free<K: Kind>(snapshot: &mut Snapshot, id: Id<K>) {
    assert!(K::table_mut(&mut snapshot.model).free(id.slot(), id.generation()));
}

/// Every report of `snapshot`, `per` slots a task, on one thread and on four.
fn both(snapshot: &Snapshot, per: u32) -> Vec<Report> {
    let per = NonZeroU32::new(per).unwrap();
    let one = checked(snapshot.model.clone(), &serial(), per);
    #[cfg(not(target_family = "wasm"))]
    {
        let four = Exec::threads(core::num::NonZeroUsize::new(4).unwrap()).unwrap();
        assert_eq!(checked(snapshot.model.clone(), &four, per), one, "four threads, {per} slots a task");
    }
    one
}

/// The reports of the checks run one after another, with no tasks and no threads: every live entity kind by kind in the
/// order of `EntityKind::ALL` and slot by slot, then every body.
fn in_order(model: &Model) -> Vec<Report> {
    let mut out = Vec::new();
    let every = |kind| {
        let to = dispatch(kind, Slots(model)).unwrap_or(0);
        dispatch(kind, Live { model, from: 0, to }).unwrap_or_default()
    };
    for &kind in EntityKind::ALL {
        for entity in every(kind) {
            check::entity(model, entity, &mut out);
        }
    }
    let members = check::members(model);
    for body in every(EntityKind::Body) {
        check::body(model, &members, Id::new(body.slot, NonZeroU32::new(body.generation).unwrap()), &mut out);
    }
    out
}

/// Each report as its reason and its entities.
fn named(reports: &[Report]) -> Vec<(Reason, Vec<EntityRef>)> {
    for report in reports {
        assert_eq!(report.code, Code::InvariantFailed);
    }
    reports.iter().map(|report| (report.reason.unwrap(), report.entities.clone())).collect()
}

#[test]
fn a_task_checks_one_chunk_of_a_table() {
    assert_eq!(usize::try_from(TASK.get()).unwrap(), CHUNK);
}

#[test]
fn a_clean_model_has_nothing_to_report() {
    let (snapshot, _) = everything();
    for per in [1, 3, 1024] {
        assert_eq!(both(&snapshot, per), [], "{per} slots a task");
    }
    assert_eq!(snapshot.check_all(&serial()), []);
    assert_eq!(Snapshot::empty(tolerance()).check_all(&serial()), []);
}

#[test]
fn every_failure_is_reported_once_kind_by_kind_then_slot_by_slot_at_any_thread_count() {
    let (mut snapshot, b) = everything();
    let (cube, frame, cylinder) = (&b[0], &b[1], &b[2]);
    let lost_curve = Edge::table(&snapshot.model).live(cylinder.edges[2].slot(), NonZeroU32::MIN).unwrap().curve;
    free(&mut snapshot, frame.faces[2]);
    free(&mut snapshot, cube.vertices[0]);
    free::<CurveGeom>(&mut snapshot, lost_curve);
    let (v0, face) = (cube.vertices[0].entity(), frame.faces[2].entity());
    let dead = Reason::DeadLink;
    let want = [
        (dead, vec![cube.edges[0].entity(), v0]),
        (dead, vec![cube.edges[3].entity(), v0]),
        (dead, vec![cube.edges[9].entity(), v0]),
        (dead, vec![cylinder.edges[2].entity(), lost_curve.entity()]),
        (dead, vec![frame.loops[2].entity(), face]),
    ];
    let reference = in_order(&snapshot.model);
    let dead: Vec<_> = named(&reference).into_iter().filter(|(reason, _)| *reason == dead).collect();
    assert_eq!(dead, want, "the dead links, in order, among the rest");
    for (k, report) in reference.iter().enumerate() {
        assert!(!reference[k + 1..].contains(report), "reported twice: {report:?}");
    }
    for per in [1, 2, 3, 7, 1024] {
        assert_eq!(both(&snapshot, per), reference, "{per} slots a task");
    }
    assert_eq!(snapshot.check_all(&serial()), reference);
}

#[test]
fn the_tasks_follow_the_kinds_then_the_bodies_each_table_in_runs() {
    let (snapshot, _) = everything();
    let per = NonZeroU32::new(10).unwrap();
    let found = tasks(&snapshot.model, per);
    let kinds: Vec<EntityKind> = found
        .iter()
        .filter_map(|task| match task {
            Task::Entities(kind, 0) => Some(*kind),
            Task::Entities(..) | Task::Bodies(_) => None,
        })
        .collect();
    let held: Vec<EntityKind> =
        EntityKind::ALL.iter().copied().filter(|kind| dispatch(*kind, Slots(&snapshot.model)) > Some(0)).collect();
    assert_eq!(held.len(), 10, "every table but the parameter curves'");
    assert_eq!(kinds, held, "every table that has a slot, in the order of EntityKind::ALL");
    let vertices = dispatch(EntityKind::Vertex, Slots(&snapshot.model)).unwrap();
    let runs: Vec<u32> = found
        .iter()
        .filter_map(|task| match task {
            Task::Entities(EntityKind::Vertex, from) => Some(*from),
            Task::Entities(..) | Task::Bodies(_) => None,
        })
        .collect();
    assert_eq!(runs, (0..vertices.div_ceil(10)).map(|run| run * 10).collect::<Vec<_>>());
    assert_eq!(found.last(), Some(&Task::Bodies(0)), "five bodies, one run, last");
    assert_eq!(tasks(&Snapshot::empty(tolerance()).model, per), []);
}
