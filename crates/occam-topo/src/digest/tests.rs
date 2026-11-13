//! Source: original
//! The digest's tests: every field and every slot's generation moves it, and nothing that leaves the model as it was
//! does.

use std::sync::Arc;

use occam_geom::{Line, Plane, UvCurve};
use occam_units::{Angle, Direction, Local, Placement, Tolerance, Vec3, Vector};

use super::*;
use crate::kind::sealed::Sealed;
use crate::kind::{ParamCurveGeom, Vertex};
use crate::models::{Built, build, cube, cylinder, frame, sheet, wire};
use crate::testing::{at, serial, tolerance, vertex};

/// The cube, the frame, the cylinder, the sheet and the wire in one snapshot, and a parameter curve, so that every
/// table holds something.
fn everything() -> (Snapshot, Vec<Built>, Id<ParamCurveGeom>) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = [cube(), frame(), cylinder(), sheet(), wire()].iter().map(|spec| build(&mut edit, spec)).collect();
    let line = Line::new(at(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
    let pcurve = edit.add(ParamCurveRecord(Arc::new(UvCurve::new(line).unwrap())));
    (edit.commit(&serial()).unwrap().0, built, pcurve)
}

/// The record of `id`, to change as no edit could.
fn record<K: Kind>(model: &mut Model, id: Id<K>) -> &mut K::Record {
    K::table_mut(model).live_mut(id.slot(), id.generation()).unwrap()
}

/// The digest of `snapshot` after `change`, made to a copy of its model.
fn after(snapshot: &Snapshot, change: &dyn Fn(&mut Model)) -> u64 {
    let mut copy = snapshot.clone();
    change(&mut copy.model);
    copy.digest()
}

/// A change to a model, as no edit could make it.
type Change<'a> = &'a dyn Fn(&mut Model);

/// Asserts that each change moves the digest.
fn each_moves(snapshot: &Snapshot, changes: &[(&str, Change)]) {
    let base = snapshot.digest();
    for (name, change) in changes {
        assert_ne!(after(snapshot, *change), base, "{name}");
    }
}

fn plane(z: f64) -> SurfaceRecord {
    let lift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, z)));
    SurfaceRecord(Arc::new(Plane::placed(&Placement::<Local, World>::identity().then(&lift))))
}

#[test]
fn the_digest_moves_with_each_field_of_a_body_a_shell_a_face_a_loop_and_a_wire() {
    let (snapshot, b, _) = everything();
    let (cube, frame, wire) = (&b[0], &b[1], &b[4]);
    let (body, shell, face, outer) = (cube.body, cube.shell.unwrap(), cube.faces[0], cube.loops[0]);
    let w = wire.wires[0];
    each_moves(
        &snapshot,
        &[
            ("body kind", &|m| record(m, body).kind = BodyKind::Sheet),
            ("body first shell", &|m| record(m, body).first_shell = None),
            ("body first wire", &|m| record(m, body).first_wire = Some(w)),
            ("body step", &|m| record(m, body).born.step = 9),
            ("body role", &|m| record(m, body).born.role = StepRole::Tool),
            ("shell body", &|m| record(m, shell).body = frame.body),
            ("shell face", &|m| record(m, shell).face = cube.faces[1]),
            ("shell next", &|m| record(m, shell).next = frame.shell),
            ("face shell", &|m| record(m, face).shell = frame.shell.unwrap()),
            ("face surface", &|m| record(m, face).surface = record(m, cube.faces[1]).surface),
            ("face sheet u", &|m| record(m, face).sheet.u = 1),
            ("face sheet v", &|m| record(m, face).sheet.v = -1),
            ("face reversed", &|m| record(m, face).reversed = true),
            ("face outer", &|m| record(m, face).outer = cube.loops[1]),
            ("face step", &|m| record(m, face).born.step = 9),
            ("face role", &|m| record(m, face).born.role = StepRole::Input),
            ("loop face", &|m| record(m, outer).face = cube.faces[1]),
            ("loop first", &|m| record(m, outer).first = cube.uses[1]),
            ("loop next", &|m| record(m, outer).next = Some(cube.loops[1])),
            ("loop witness u", &|m| record(m, outer).witness = UvPoint::new(0.25, 0.0)),
            ("loop witness v", &|m| record(m, outer).witness = UvPoint::new(0.0, 0.25)),
            ("wire body", &|m| record(m, w).body = cube.body),
            ("wire first", &|m| record(m, w).first = wire.uses[1]),
            ("wire next", &|m| record(m, w).next = Some(w)),
        ],
    );
}

#[test]
fn the_digest_moves_with_each_field_of_an_edge_use_and_an_edge() {
    let (snapshot, b, pcurve) = everything();
    let (cube, wire) = (&b[0], &b[4]);
    let (u, e) = (cube.uses[0], cube.edges[0]);
    let flip = |end| if end == End::Start { End::Finish } else { End::Start };
    each_moves(
        &snapshot,
        &[
            ("use edge", &|m| record(m, u).edge = cube.edges[1]),
            ("use owner kind", &|m| record(m, u).owner = UseOwner::Wire(wire.wires[0])),
            ("use owner", &|m| record(m, u).owner = UseOwner::Loop(cube.loops[1])),
            ("use next", &|m| record(m, u).next = None),
            ("use radial", &|m| record(m, u).radial = cube.uses[3]),
            ("use reversed", &|m| record(m, u).reversed = !record(m, u).reversed),
            ("use pcurve", &|m| record(m, u).pcurve = Some(pcurve)),
            ("edge curve", &|m| record(m, e).curve = record(m, cube.edges[1]).curve),
            ("edge range low", &|m| record(m, e).range.0 = -1.0),
            ("edge range high", &|m| record(m, e).range.1 = 7.0),
            ("edge start", &|m| record(m, e).ends.0 = cube.vertices[5]),
            ("edge finish", &|m| record(m, e).ends.1 = cube.vertices[5]),
            ("edge around start", &|m| record(m, e).around.0.edge = cube.edges[7]),
            ("edge around start end", &|m| record(m, e).around.0.end = flip(record(m, e).around.0.end)),
            ("edge around finish", &|m| record(m, e).around.1.edge = cube.edges[7]),
            ("edge around finish end", &|m| record(m, e).around.1.end = flip(record(m, e).around.1.end)),
            ("edge first use", &|m| record(m, e).first_use = None),
            ("edge role", &|m| record(m, e).role = EdgeRole::Seam),
            ("edge deviation", &|m| record(m, e).deviation = Length::from_millimetres(1e-3)),
            ("edge step", &|m| record(m, e).born.step = 9),
            ("edge role made", &|m| record(m, e).born.role = StepRole::Profile),
        ],
    );
}

#[test]
fn the_digest_moves_with_each_field_of_a_vertex_each_geometry_and_the_model() {
    let (snapshot, b, pcurve) = everything();
    let cube = &b[0];
    let v = cube.vertices[0];
    let moved = |x: f64, y: f64, z: f64| move |m: &mut Model| record(m, v).point = at(x, y, z);
    let star = |m: &mut Model| record(m, v).star.unwrap();
    let surface = |m: &mut Model| record(m, cube.faces[0]).surface;
    let curve = |m: &mut Model| record(m, cube.edges[0]).curve;
    let other = UvCurve::new(Line::new(at(0.0, 2.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)))).unwrap();
    each_moves(
        &snapshot,
        &[
            ("vertex x", &moved(0.5, 0.0, 0.0)),
            ("vertex y", &moved(0.0, 0.5, 0.0)),
            ("vertex z", &moved(0.0, 0.0, 0.5)),
            ("vertex radius", &|m| record(m, v).radius = Length::from_millimetres(1e-9)),
            ("vertex star", &|m| record(m, v).star = None),
            ("vertex star edge", &|m| record(m, v).star = Some(Around { edge: cube.edges[5], ..star(m) })),
            ("vertex star end", &|m| record(m, v).star = Some(Around { end: End::Finish, ..star(m) })),
            ("surface", &|m| {
                let id = surface(m);
                *record(m, id) = plane(0.5);
            }),
            ("curve", &|m| {
                let id = curve(m);
                *record(m, id) = CurveRecord(Arc::new(Line::new(
                    at(0.0, 0.0, 3.0),
                    Direction::normalised(Vec3::new(0.0, 1.0, 0.0)),
                )));
            }),
            ("parameter curve", &|m| *record(m, pcurve) = ParamCurveRecord(Arc::new(other.clone()))),
            ("tolerance length", &|m| m.tolerance = tolerance_of(1e-6, 1e-9)),
            ("tolerance angle", &|m| m.tolerance = tolerance_of(1e-7, 1e-8)),
            ("step", &|m| m.step = 2),
        ],
    );
}

#[test]
fn a_clone_a_dropped_edit_and_a_failed_commit_leave_the_digest_and_the_same_build_gives_it_again() {
    let (snapshot, _, _) = everything();
    let digest = snapshot.digest();
    assert_eq!(snapshot.clone().digest(), digest);
    let mut dropped = snapshot.edit();
    build(&mut dropped, &cube());
    drop(dropped);
    let mut failed = snapshot.edit();
    let _: Id<Vertex> = failed.reserve();
    assert!(failed.commit(&serial()).is_err());
    assert_eq!(snapshot.digest(), digest);
    assert_eq!(everything().0.digest(), digest, "built again, the same digest");
    assert_ne!(Snapshot::empty(tolerance()).digest(), digest);
}

/// A model of vertices written straight into the table, each `Some(x)` live at `x` and each `None` freed, at `step`.
fn vertices(slots: &[Option<f64>], step: u64) -> Snapshot {
    let mut snapshot = Snapshot::empty(tolerance());
    snapshot.model.step = step;
    let table = Vertex::table_mut(&mut snapshot.model);
    for x in slots {
        let (slot, generation) = table.reserve().unwrap();
        assert!(table.put(slot, generation, vertex(x.unwrap_or(0.0))));
        if x.is_none() {
            assert!(table.free(slot, generation));
        }
    }
    snapshot
}

#[test]
fn every_slot_counts_free_or_live_with_its_generation() {
    let one = vertices(&[Some(1.0)], 2);
    assert_ne!(vertices(&[Some(1.0), None], 2).digest(), one.digest(), "a free slot after it");
    assert_ne!(vertices(&[None, Some(1.0)], 2).digest(), one.digest(), "the same vertex in slot 1");
    let mut reused = vertices(&[None], 2);
    let table = Vertex::table_mut(&mut reused.model);
    let (slot, generation) = table.reserve().unwrap();
    assert!(table.put(slot, generation, vertex(1.0)));
    assert_eq!((slot, generation.get()), (0, 2));
    assert_ne!(reused.digest(), one.digest(), "the same vertex in slot 0 at generation 2");
    let mut reserved = vertices(&[Some(1.0)], 2);
    assert!(Vertex::table_mut(&mut reserved.model).reserve().is_some());
    assert_ne!(reserved.digest(), vertices(&[Some(1.0), None], 2).digest(), "reserved, not free, at generation 1");
    assert_ne!(vertices(&[Some(1.0)], 3).digest(), one.digest(), "a later step");
    assert_ne!(vertices(&[Some(1.0), Some(1.0)], 2).digest(), one.digest());
    assert_eq!(vertices(&[Some(1.0), None], 2).digest(), vertices(&[Some(1.0), None], 2).digest());
}

#[test]
fn the_same_live_records_with_another_free_generation_differ_and_give_the_next_edit_other_ids() {
    let once = vertices(&[Some(1.0), None], 2);
    let mut twice = vertices(&[Some(1.0), None], 2);
    let table = Vertex::table_mut(&mut twice.model);
    let (slot, generation) = table.reserve().unwrap();
    assert!(table.put(slot, generation, vertex(9.0)) && table.free(slot, generation));
    assert_eq!(once.ids::<Vertex>(), twice.ids::<Vertex>(), "the same live records");
    assert_ne!(once.digest(), twice.digest());
    let next = |snapshot: &Snapshot| snapshot.edit().add(vertex(2.0));
    let (a, b) = (next(&once), next(&twice));
    assert_eq!((a.slot(), a.generation().get(), b.slot(), b.generation().get()), (1, 2, 1, 3));
}

#[test]
fn every_nan_is_one_pattern_and_the_two_zeros_differ() {
    let at_x = |x: f64| vertices(&[Some(x)], 1).digest();
    let quiet = at_x(f64::from_bits(0x7FF8_0000_0000_0000));
    assert_eq!(at_x(f64::from_bits(0xFFF8_0000_0000_0000)), quiet, "the sign set");
    assert_eq!(at_x(f64::from_bits(0x7FF0_0000_0000_0001)), quiet, "signalling");
    assert_eq!(at_x(f64::from_bits(0x7FF8_DEAD_BEEF_0001)), quiet, "a payload");
    assert_ne!(at_x(0.0), at_x(-0.0), "the zeros, as occam-math's canonical bits keep them");
    assert_ne!(at_x(0.0), quiet);
}

#[test]
fn the_order_of_the_slots_counts() {
    let digest = |xs: &[f64]| vertices(&xs.iter().copied().map(Some).collect::<Vec<_>>(), 1).digest();
    assert_ne!(digest(&[1.0, 2.0]), digest(&[2.0, 1.0]));
    assert_ne!(digest(&[1.0]), digest(&[1.0, 1.0]));
}

/// A tolerance of `millimetres` and `radians`.
fn tolerance_of(millimetres: f64, radians: f64) -> Tolerance {
    Tolerance::new(Length::from_millimetres(millimetres), Angle::from_radians(radians)).unwrap()
}
