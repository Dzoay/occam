//! Source: original
//! What a commit reaches for the checks that read geometry: a change to the edges and vertices of a loop reaches the
//! face the loop bounds, and a curve or surface replaced in place reaches everything on it, so that a commit finds what
//! the whole-model check finds, and measures again every deviation the change moves.

use std::sync::Arc;

use occam_geom::{Line, Plane};
use occam_report::{EntityRef, Reason, Report};
use occam_units::{Direction, Local, Placement, Point, Vec3, Vector, World};

use crate::check;
use crate::edit::Edit;
use crate::kind::sealed::Sealed;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Kind, Loop, Shell, Vertex, Wire};
use crate::models::{Built, Spec, build, cube, holed_sheet, point};
use crate::records::{CurveRecord, SurfaceRecord};
use crate::snapshot::{Model, Snapshot};
use crate::testing::{serial, tolerance};

/// A failure's reason and the entities it names.
type Failure = (Reason, Vec<EntityRef>);

/// The failures `reports` give, sorted.
fn failures(reports: &[Report]) -> Vec<Failure> {
    let mut found: Vec<Failure> = reports.iter().map(|r| (r.reason.unwrap(), r.entities.clone())).collect();
    found.sort();
    found
}

/// Every live entity of kind `K` in `m`.
fn live<K: Kind>(m: &Model) -> impl Iterator<Item = EntityRef> + '_ {
    K::table(m).records().map(|(slot, generation, _)| Id::<K>::new(slot, generation).entity())
}

/// The whole-model check: every entity, then every body.
fn whole(m: &Model) -> Vec<Report> {
    let mut out = Vec::new();
    let entities: Vec<EntityRef> = live::<Body>(m)
        .chain(live::<Shell>(m))
        .chain(live::<Face>(m))
        .chain(live::<Loop>(m))
        .chain(live::<Wire>(m))
        .chain(live::<EdgeUse>(m))
        .chain(live::<Edge>(m))
        .chain(live::<Vertex>(m))
        .collect();
    for entity in entities {
        check::entity(m, entity, &mut out);
    }
    let members = check::members(m);
    for (slot, generation, _) in Body::table(m).records() {
        check::body(m, &members, Id::new(slot, generation), &mut out);
    }
    out
}

/// `spec` committed clean, then the snapshot and what it built.
fn committed(spec: &Spec) -> (Snapshot, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    (edit.commit(&serial()).unwrap().0, built)
}

/// `spec` committed clean, then `change` made in a second edit: what its commit reports, and what the whole-model check
/// finds in the model it was given.
fn changed(spec: &Spec, change: impl FnOnce(&mut Edit, &Built)) -> (Vec<Failure>, Vec<Failure>) {
    let (snapshot, built) = committed(spec);
    let mut edit = snapshot.edit();
    change(&mut edit, &built);
    let next = edit.next.clone();
    let commit = edit.commit(&serial()).map_or_else(|reports| failures(&reports), |_| Vec::new());
    (commit, failures(&whole(&next)))
}

/// The placement with no turn whose origin is `p`.
fn at(p: Vec3) -> Placement<Local, World> {
    Placement::<Local, World>::identity().then(&Placement::<World, World>::translation(Vector::from_vec3(p)))
}

#[test]
fn a_hole_moved_by_its_edges_and_vertices_alone_reaches_the_face_it_is_in() {
    let (commit, whole) = changed(&holed_sheet(), |edit, built| {
        let moved = |x: f64| (x - 1.0) * 0.5 + 0.25;
        for &v in &built.vertices[4..8] {
            let mut record = edit.record(v).unwrap();
            let p = record.point.to_vec3();
            record.point = point(moved(p.x), moved(p.y), 0.0);
            edit.set(v, record).unwrap();
        }
        for &e in &built.edges[4..8] {
            let mut record = edit.record(e).unwrap();
            let end = |v| edit.record::<Vertex>(v).unwrap().point.to_vec3();
            let (a, b) = (end(record.ends.0), end(record.ends.1));
            let line = Line::new(Point::from_vec3(a), Direction::normalised(b - a));
            record.curve = edit.add(CurveRecord(Arc::new(line)));
            record.range = (0.0, (b - a).norm());
            edit.set(e, record).unwrap();
        }
    });
    let reasons: Vec<Reason> = whole.iter().map(|(reason, _)| *reason).collect();
    assert!(reasons.contains(&Reason::LoopsNotNested) && reasons.contains(&Reason::WitnessNotInside), "{whole:?}");
    assert_eq!(commit, whole);
}

#[test]
fn a_surface_replaced_in_place_reaches_everything_on_it() {
    let (commit, whole) = changed(&cube(), |edit, built| {
        let top = edit.record(built.faces[1]).unwrap().surface;
        edit.set(top, SurfaceRecord(Arc::new(Plane::placed(&at(Vec3::new(10.5, 0.5, 1.0)))))).unwrap();
    });
    assert!(whole.iter().any(|(reason, _)| *reason == Reason::WitnessNotInside), "{whole:?}");
    assert_eq!(commit, whole);
}

#[test]
fn a_surface_replaced_in_place_has_the_deviations_of_its_edges_measured_again() {
    let (snapshot, built) = committed(&cube());
    let mut edit = snapshot.edit();
    let top = edit.record(built.faces[1]).unwrap().surface;
    edit.set(top, SurfaceRecord(Arc::new(Plane::placed(&at(Vec3::new(0.5, 0.5, 1.0 + 5e-8)))))).unwrap();
    let (after, changes) = edit.commit(&serial()).unwrap();
    let on_top = after.view(built.faces[1]).unwrap().edges();
    assert_eq!(on_top.len(), 4);
    for &e in &built.edges {
        let deviation = after.view(e).unwrap().deviation().millimetres();
        if on_top.contains(&e) {
            assert!((4e-8..6e-8).contains(&deviation), "{e:?} {deviation}");
            assert!(changes.changed.contains(&e.entity()), "{e:?}");
        } else {
            let before = snapshot.view(e).unwrap().deviation().millimetres();
            assert_eq!(deviation.to_bits(), before.to_bits(), "{e:?}");
        }
    }
}
