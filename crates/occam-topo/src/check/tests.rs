//! Source: original
//! The checker's tests: every clean model commits and passes the whole-model checks; each invariant has a seeded
//! corruption of a clean model that a commit and the whole-model checks both catch, naming the entities; and what a
//! commit reads follows the edit, not the model.
#![expect(clippy::arithmetic_side_effects, reason = "small indices in tests")]

use core::f64::consts::SQRT_2;
use std::sync::Arc;

use occam_geom::Line;
use occam_report::StepRole;
use occam_units::{Direction, Length, UvPoint};

use super::read::reads;
use super::*;
use crate::edit::Changes;
use crate::kind::{CurveGeom, Edge, EdgeUse, Face, Loop, Record, Shell, Vertex};
use crate::models::{Built, Spec, build, cube, cylinder, frame, join_star, pinched, plate, point, sheet, wire};
use crate::records::{
    Around, BodyKind, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord, ShellRecord,
    UseOwner, VertexRecord,
};
use crate::snapshot::Snapshot;
use crate::testing::{serial, tolerance, x_axis};

/// A failure's reason and the entities it names.
pub(super) type Failure = (Reason, Vec<EntityRef>);

/// The failures `reports` give, sorted.
pub(super) fn failures(reports: &[Report]) -> Vec<Failure> {
    let mut found: Vec<Failure> = reports
        .iter()
        .map(|r| {
            assert_eq!((r.code, r.fault), (Code::InvariantFailed, Some(Fault::Internal)), "{r:?}");
            (r.reason.unwrap(), r.entities.clone())
        })
        .collect();
    found.sort();
    found
}

/// Every live entity of kind `K`.
fn all<K: Kind>(model: &Model) -> Vec<Id<K>> {
    K::table(model).records().map(|(slot, generation, _)| Id::new(slot, generation)).collect()
}

/// What the whole-model checks find: [`entity`] over every live entity, then [`body`] over every body.
pub(super) fn whole(m: &Model) -> Vec<Report> {
    let mut out = Vec::new();
    let entities = [
        all::<Body>(m).into_iter().map(Id::entity).collect::<Vec<_>>(),
        all::<Shell>(m).into_iter().map(Id::entity).collect(),
        all::<Face>(m).into_iter().map(Id::entity).collect(),
        all::<Loop>(m).into_iter().map(Id::entity).collect(),
        all::<crate::kind::Wire>(m).into_iter().map(Id::entity).collect(),
        all::<EdgeUse>(m).into_iter().map(Id::entity).collect(),
        all::<Edge>(m).into_iter().map(Id::entity).collect(),
        all::<Vertex>(m).into_iter().map(Id::entity).collect(),
    ];
    for e in entities.into_iter().flatten() {
        entity(m, e, &mut out);
    }
    let members = members(m);
    for b in all::<Body>(m) {
        body(m, &members, b, &mut out);
    }
    out
}

/// `spec` built and committed.
pub(super) fn clean(spec: &Spec) -> (Snapshot, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    (edit.commit(&serial()).unwrap().0, built)
}

/// `spec` built clean, then `corrupt` made in a second edit: what its commit reports, and what the whole-model checks
/// find in the model it refused, as measured.
pub(super) fn seeded(spec: &Spec, corrupt: impl FnOnce(&mut Edit, &Built)) -> (Vec<Failure>, Vec<Failure>) {
    let (snapshot, built) = clean(spec);
    let mut edit = snapshot.edit();
    corrupt(&mut edit, &built);
    // Commit measures each changed edge's deviation before it checks; the whole-model checks read what is stored, so
    // they are given the model as measured.
    let _ = crate::geometric::measure(&mut edit);
    let next = edit.next.clone();
    (failures(&edit.commit(&serial()).unwrap_err()), failures(&whole(&next)))
}

/// Changes the record of `id` in `edit`.
pub(super) fn change<R: Record>(edit: &mut Edit, id: Id<R::Kind>, how: impl FnOnce(&mut R)) {
    let mut record = edit.record(id).unwrap();
    how(&mut record);
    edit.set(id, record).unwrap();
}

/// The uses of a loop or wire from `first`, in `edit`.
fn uses(edit: &Edit, first: Id<EdgeUse>) -> Vec<Id<EdgeUse>> {
    let mut found = vec![first];
    while let Some(next) = edit.record(*found.last().unwrap()).unwrap().next.filter(|n| *n != first) {
        found.push(next);
    }
    found
}

/// The uses of the outer loop of `face`, in `edit`.
pub(super) fn face_uses(edit: &Edit, face: Id<Face>) -> Vec<Id<EdgeUse>> {
    let outer = edit.record(face).unwrap().outer;
    uses(edit, edit.record(outer).unwrap().first)
}

/// The uses of `edge` round its ring, from its first, in `edit`.
fn ring(edit: &Edit, edge: Id<Edge>) -> Vec<Id<EdgeUse>> {
    let first = edit.record(edge).unwrap().first_use.unwrap();
    let mut found = vec![first];
    while let Some(next) = Some(edit.record(*found.last().unwrap()).unwrap().radial).filter(|n| *n != first) {
        found.push(next);
    }
    found
}

/// The places round the star of `vertex`, from the one it names, in `edit`.
fn star_places(edit: &Edit, vertex: Id<Vertex>) -> Vec<Around> {
    let first = edit.record(vertex).unwrap().star.unwrap();
    let around = |p: Around| {
        let e = edit.record(p.edge).unwrap();
        if p.end == End::Start { e.around.0 } else { e.around.1 }
    };
    let mut found = vec![first];
    while around(*found.last().unwrap()) != first {
        found.push(around(*found.last().unwrap()));
    }
    found
}

/// Moves `faces` of a body built as one shell into a second shell of the same body, which begins from the first of them.
pub(super) fn second_shell(edit: &mut Edit, built: &Built, faces: &[usize]) -> Id<Shell> {
    let (first, shell) = (built.shell.unwrap(), edit.reserve::<Shell>());
    let moved: Vec<Id<Face>> = faces.iter().map(|&k| built.faces[k]).collect();
    let kept: Vec<Id<Face>> = built.faces.iter().copied().filter(|f| !moved.contains(f)).collect();
    for &f in &moved {
        change(edit, f, |r: &mut FaceRecord| r.shell = shell);
    }
    change(edit, first, |r: &mut ShellRecord| (r.face, r.next) = (kept[0], Some(shell)));
    edit.set(shell, ShellRecord { body: built.body, face: moved[0], next: None }).unwrap();
    shell
}

#[test]
fn every_clean_model_commits_and_passes_the_whole_model_checks() {
    for spec in [cube(), frame(), cylinder(), sheet(), wire(), plate(3)] {
        let (snapshot, _) = clean(&spec);
        assert_eq!(failures(&whole(&snapshot.model)), [], "{:?}", spec.kind);
    }
}

#[test]
fn a_loop_that_does_not_come_round_is_not_closed() {
    let mut named = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let uses = face_uses(edit, built.faces[0]);
        change(edit, uses[3], |r: &mut EdgeUseRecord| r.next = None);
        named = vec![built.loops[0].entity(), uses[3].entity()];
    });
    assert_eq!(found, (vec![(Reason::LoopNotClosed, named.clone())], vec![(Reason::LoopNotClosed, named)]));
}

#[test]
fn uses_out_of_order_do_not_meet() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let u = face_uses(edit, built.faces[0]);
        for (from, to) in [(0, 2), (2, 1), (1, 3)] {
            change(edit, u[from], |r: &mut EdgeUseRecord| r.next = Some(u[to]));
            expected.push((Reason::UsesDoNotMeet, vec![u[from].entity(), u[to].entity()]));
        }
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_use_given_to_another_loop_is_listed_elsewhere_and_not_listed_there() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let u = face_uses(edit, built.faces[0])[1];
        change(edit, u, |r: &mut EdgeUseRecord| r.owner = UseOwner::Loop(built.loops[1]));
        // The bottom edge it uses now bounds the top face too, a millimetre off it.
        let e = edit.record(u).unwrap().edge;
        let bounded = vec![e.entity(), built.faces[1].entity(), built.faces[3].entity()];
        expected = vec![
            (Reason::ListedElsewhere, vec![built.loops[0].entity(), u.entity()]),
            (Reason::NotListed, vec![u.entity(), built.loops[1].entity()]),
            (Reason::EdgeOffFace, bounded),
        ];
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_broken_ring_of_uses_round_an_edge_is_found() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let [_, second] = ring(edit, built.edges[0])[..] else { panic!("two uses") };
        change(edit, second, |r: &mut EdgeUseRecord| r.radial = second);
        expected = vec![(Reason::ListBroken, vec![built.edges[0].entity(), second.entity()])];
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_broken_star_and_an_edge_left_out_of_its_star_are_found() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let [_, p1, _] = star_places(edit, built.vertices[0])[..] else { panic!("three edges at a corner") };
        change(
            edit,
            p1.edge,
            |r: &mut EdgeRecord| if p1.end == End::Start { r.around.0 = p1 } else { r.around.1 = p1 },
        );
        expected = vec![(Reason::ListBroken, vec![built.vertices[0].entity(), p1.edge.entity()])];
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
    let found = seeded(&cube(), |edit, built| {
        let [p0, p1, p2] = star_places(edit, built.vertices[0])[..] else { panic!("three edges at a corner") };
        change(
            edit,
            p0.edge,
            |r: &mut EdgeRecord| if p0.end == End::Start { r.around.0 = p2 } else { r.around.1 = p2 },
        );
        expected = vec![(Reason::NotListed, vec![p1.edge.entity(), built.vertices[0].entity()])];
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_face_turned_round_leaves_each_of_its_edges_with_uses_running_the_same_way() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let u = face_uses(edit, built.faces[0]);
        for (k, &x) in u.iter().enumerate() {
            let before = u[(k + 3) % 4];
            change(edit, x, |r: &mut EdgeUseRecord| (r.next, r.reversed) = (Some(before), !r.reversed));
        }
        for &x in &u {
            let e = edit.record(x).unwrap().edge;
            let named = [e.entity()].into_iter().chain(ring(edit, e).into_iter().map(Id::entity)).collect();
            expected.push((Reason::UsesNotOpposed, named));
        }
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn an_edge_has_the_uses_its_bodys_kind_allows() {
    let mut expected = Vec::new();
    let found = seeded(&sheet(), |edit, built| {
        change(edit, built.body, |r: &mut crate::records::BodyRecord| r.kind = BodyKind::Solid);
        let uses = |e: Id<Edge>| (Reason::WrongUseCount, vec![e.entity(), built.body.entity()]);
        expected = built.edges.iter().map(|&e| uses(e)).collect();
        expected.push((Reason::GenusNotWhole, vec![built.shell.unwrap().entity()]));
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
    let found = seeded(&wire(), |edit, built| {
        change(edit, built.body, |r: &mut crate::records::BodyRecord| r.kind = BodyKind::Solid);
        let uses = |e: Id<Edge>| (Reason::WrongUseCount, vec![e.entity(), built.body.entity()]);
        expected = built.edges.iter().map(|&e| uses(e)).collect();
        expected.push((Reason::BodyKindMismatch, vec![built.body.entity()]));
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_body_that_lists_no_shell_neither_fits_its_kind_nor_lists_its_shell() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        change(edit, built.body, |r: &mut crate::records::BodyRecord| r.first_shell = None);
        let shell = built.shell.unwrap().entity();
        expected = vec![
            (Reason::NotListed, vec![shell, built.body.entity()]),
            (Reason::BodyKindMismatch, vec![built.body.entity()]),
        ];
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_body_or_a_vertex_that_still_names_what_was_removed_has_a_dead_link() {
    let (commit, whole) = seeded(&cube(), |edit, built| edit.remove(built.shell.unwrap()).unwrap());
    let (body, shell) = (crate::kind::Body::ENTITY, Shell::ENTITY);
    for found in [&commit, &whole] {
        let dead = found.iter().find(|(r, e)| *r == Reason::DeadLink && e[0].kind == body && e[1].kind == shell);
        assert!(dead.is_some(), "the body still names the shell: {found:?}");
    }
    let (commit, whole) = seeded(&cube(), |edit, built| {
        let e = built.edges[0];
        let v = edit.record(e).unwrap().ends.0;
        change(edit, v, |r: &mut VertexRecord| r.star = Some(Around { edge: e, end: End::Start }));
        edit.remove(e).unwrap();
    });
    let vertex = Vertex::ENTITY;
    for found in [&commit, &whole] {
        let dead =
            found.iter().find(|(r, e)| *r == Reason::DeadLink && e[0].kind == vertex && e[1].kind == Edge::ENTITY);
        assert!(dead.is_some(), "the vertex's star still names the edge: {found:?}");
    }
}

#[test]
fn the_uses_of_a_wire_out_of_order_do_not_meet() {
    let mut expected = Vec::new();
    let found = seeded(&wire(), |edit, built| {
        let u = &built.uses;
        change(edit, u[0], |r: &mut EdgeUseRecord| r.next = Some(u[2]));
        change(edit, u[2], |r: &mut EdgeUseRecord| r.next = Some(u[1]));
        change(edit, u[1], |r: &mut EdgeUseRecord| r.next = None);
        expected = vec![
            (Reason::UsesDoNotMeet, vec![u[0].entity(), u[2].entity()]),
            (Reason::UsesDoNotMeet, vec![u[2].entity(), u[1].entity()]),
        ];
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

/// A second wire body in `edit`, its one wire using the first edge of the wire `built` with a use of its own, in the
/// same ring of uses as the first: the body.
fn second_wire_body(edit: &mut Edit, built: &Built) -> Id<crate::kind::Body> {
    let (e, u0) = (built.edges[0], built.uses[0]);
    let (body, wire, u) = (edit.reserve::<crate::kind::Body>(), edit.reserve::<crate::kind::Wire>(), edit.reserve());
    let born = edit.provenance(StepRole::Made);
    edit.set(
        body,
        crate::records::BodyRecord { kind: BodyKind::Wire, first_shell: None, first_wire: Some(wire), born },
    )
    .unwrap();
    edit.set(wire, crate::records::WireRecord { body, first: u, next: None }).unwrap();
    let owner = UseOwner::Wire(wire);
    edit.set(u, EdgeUseRecord { edge: e, owner, next: None, radial: u0, reversed: false, pcurve: None }).unwrap();
    change(edit, u0, |r: &mut EdgeUseRecord| r.radial = u);
    body
}

#[test]
fn an_edge_of_two_wire_bodies_is_shared_and_names_both() {
    let mut expected = Vec::new();
    let found = seeded(&wire(), |edit, built| {
        let second = second_wire_body(edit, built);
        let mut bodies = [built.body.entity(), second.entity()];
        bodies.sort();
        expected = vec![(Reason::EdgeShared, vec![built.edges[0].entity(), bodies[0], bodies[1]])];
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_wire_body_with_a_shell_too_does_not_fit_its_kind() {
    let mut second = None;
    let (commit, whole) = seeded(&wire(), |edit, built| {
        let body = second_wire_body(edit, built);
        let shell =
            edit.add(ShellRecord { body, face: crate::kind::Id::new(0, core::num::NonZeroU32::MIN), next: None });
        change(edit, body, |r: &mut crate::records::BodyRecord| r.first_shell = Some(shell));
        second = Some(body.entity());
    });
    let mismatch = (Reason::BodyKindMismatch, vec![second.unwrap()]);
    assert!(commit.contains(&mismatch) && whole.contains(&mismatch), "{commit:?} {whole:?}");
}

#[test]
fn an_edge_between_faces_of_two_shells_is_shared_and_neither_shell_closes() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let second = second_shell(edit, built, &[5]);
        let first = built.shell.unwrap().entity();
        for x in face_uses(edit, built.faces[5]) {
            let e = edit.record(x).unwrap().edge.entity();
            expected.push((Reason::EdgeShared, vec![e, first, second.entity()]));
        }
        expected.push((Reason::GenusNotWhole, vec![first]));
        expected.push((Reason::GenusNotWhole, vec![second.entity()]));
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_closed_shell_whose_counts_give_no_whole_genus_is_found() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let p =
            edit.add(VertexRecord { point: point(0.5, 0.5, 1.0), radius: Length::from_millimetres(0.0), star: None });
        let curve: Id<CurveGeom> = edit.add(x_axis());
        let (c, x1, x2, hole) = (edit.reserve::<Edge>(), edit.reserve(), edit.reserve(), edit.reserve::<Loop>());
        let (start, finish) = (Around { edge: c, end: End::Start }, Around { edge: c, end: End::Finish });
        let born = edit.provenance(StepRole::Made);
        let deviation = Length::from_millimetres(0.0);
        let role = EdgeRole::Ordinary;
        let record = EdgeRecord {
            curve,
            range: (0.0, 1.0),
            ends: (p, p),
            around: (finish, start),
            first_use: Some(x1),
            role,
            deviation,
            born,
        };
        edit.set(c, record).unwrap();
        change(edit, p, |r: &mut VertexRecord| r.star = Some(start));
        let owner = UseOwner::Loop(hole);
        let a_use =
            |next, radial, reversed| EdgeUseRecord { edge: c, owner, next: Some(next), radial, reversed, pcurve: None };
        edit.set(x1, a_use(x2, x2, false)).unwrap();
        edit.set(x2, a_use(x1, x1, true)).unwrap();
        let top = built.faces[1];
        edit.set(hole, LoopRecord { face: top, first: x1, next: None, witness: UvPoint::new(0.0, 0.0) }).unwrap();
        change(edit, built.loops[1], |r: &mut LoopRecord| r.next = Some(hole));
        expected = vec![(Reason::GenusNotWhole, vec![built.shell.unwrap().entity()])];
    });
    // The seeded loop is one edge used twice, which no geometry bounds: its vertex is off the curve it borrows, which is
    // off the face, and its witness cannot be inside it. Only the checker's own reasons are compared.
    let geometric =
        |reason: &Reason| matches!(reason, Reason::VertexOffEdge | Reason::WitnessNotInside | Reason::EdgeOffFace);
    let own = |found: Vec<Failure>| found.into_iter().filter(|(reason, _)| !geometric(reason)).collect::<Vec<_>>();
    assert!(found.0.iter().chain(&found.1).any(|(reason, _)| geometric(reason)), "geometry fails too: {found:?}");
    assert_eq!((own(found.0), own(found.1)), (expected.clone(), expected));
}

#[test]
fn two_cubes_meeting_at_a_corner_are_two_shells_not_one() {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, &pinched());
    let next = edit.next.clone();
    let shell = built.shell.unwrap().entity();
    // The faces joined to the one the shell begins from are one cube, whose counts are whole: only the joining fails.
    let expected = vec![(Reason::ShellNotConnected, vec![shell, built.faces[6].entity()])];
    assert_eq!(failures(&edit.commit(&serial()).unwrap_err()), expected);
    assert_eq!(failures(&whole(&next)), expected);
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, &pinched());
    second_shell(&mut edit, &built, &[6, 7, 8, 9, 10, 11]);
    let (snapshot, _) = edit.commit(&serial()).unwrap();
    assert_eq!(failures(&whole(&snapshot.model)), [], "the corner they share counts once in each");
}

#[test]
fn removing_a_vertex_finds_every_edge_still_ending_there() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let v = built.vertices[0];
        for p in star_places(edit, v) {
            expected.push((Reason::DeadLink, vec![p.edge.entity(), v.entity()]));
        }
        edit.remove(v).unwrap();
        expected.sort();
    });
    assert_eq!(found.0.len(), 3, "three edges meet at a corner of a cube");
    assert_eq!(found, (expected.clone(), expected.clone()));
}

#[test]
fn a_use_of_an_edge_that_lists_no_uses_is_not_listed() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        let e = built.edges[0];
        expected = ring(edit, e).into_iter().map(|u| (Reason::NotListed, vec![u.entity(), e.entity()])).collect();
        change(edit, e, |r: &mut EdgeRecord| r.first_use = None);
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected.clone()));
}

/// Splits the square `face` of a plate's top by its diagonal into two triangles, in `edit`. The new triangle joins the
/// plate's shell by naming it and sharing the diagonal.
pub(super) fn split(edit: &mut Edit, face: Id<Face>) {
    let (square, sides) = (edit.record(face).unwrap(), face_uses(edit, face));
    let ends = |edit: &Edit, x: Id<EdgeUse>| {
        let (side, edge) = (edit.record(x).unwrap(), edit.record(edit.record(x).unwrap().edge).unwrap());
        if side.reversed { (edge.ends.1, edge.ends.0) } else { edge.ends }
    };
    let (from, to) = (ends(edit, sides[0]).0, ends(edit, sides[1]).1);
    let at = edit.record(from).unwrap().point;
    let towards = edit.record(to).unwrap().point.to_vec3() - at.to_vec3();
    let curve = edit.add(CurveRecord(Arc::new(Line::new(at, Direction::normalised(towards)))));
    let (diagonal, da, db, other, half) =
        (edit.reserve::<Edge>(), edit.reserve(), edit.reserve(), edit.reserve::<Loop>(), edit.reserve::<Face>());
    let around = (
        join_star(edit, from, Around { edge: diagonal, end: End::Start }),
        join_star(edit, to, Around { edge: diagonal, end: End::Finish }),
    );
    let (born, deviation, role) = (edit.provenance(StepRole::Made), Length::from_millimetres(0.0), EdgeRole::Ordinary);
    let record = EdgeRecord {
        curve,
        range: (0.0, SQRT_2),
        ends: (from, to),
        around,
        first_use: Some(da),
        role,
        deviation,
        born,
    };
    edit.set(diagonal, record).unwrap();
    let a_use = |owner, next, radial, reversed| EdgeUseRecord {
        edge: diagonal,
        owner,
        next: Some(next),
        radial,
        reversed,
        pcurve: None,
    };
    edit.set(da, a_use(UseOwner::Loop(square.outer), sides[0], db, true)).unwrap();
    edit.set(db, a_use(UseOwner::Loop(other), sides[2], da, false)).unwrap();
    change(edit, sides[1], |r: &mut EdgeUseRecord| r.next = Some(da));
    change(edit, sides[2], |r: &mut EdgeUseRecord| r.owner = UseOwner::Loop(other));
    change(edit, sides[3], |r: &mut EdgeUseRecord| (r.owner, r.next) = (UseOwner::Loop(other), Some(db)));
    change(edit, square.outer, |r: &mut LoopRecord| r.witness = UvPoint::new(1.0 / 6.0, -1.0 / 6.0));
    let witness = UvPoint::new(-1.0 / 6.0, 1.0 / 6.0);
    edit.set(other, LoopRecord { face: half, first: sides[2], next: None, witness }).unwrap();
    edit.set(half, FaceRecord { outer: other, born, ..square }).unwrap();
}

/// Splits the middle top square of a plate `k` squares across by its diagonal into two triangles, and commits it.
pub(super) fn split_square(snapshot: &Snapshot, built: &Built, k: usize) -> Result<(Snapshot, Changes), Vec<Report>> {
    let mut edit = snapshot.edit();
    split(&mut edit, built.faces[k * k + (k / 2) * k + k / 2]);
    edit.commit(&serial())
}

#[test]
fn what_a_commit_reads_follows_the_edit_not_the_model() {
    let read = |k: usize| {
        let (snapshot, built) = clean(&plate(k));
        assert_eq!(snapshot.ids::<Face>().len(), 2 * k * k + 4 * k);
        let before = reads();
        let (after, changes) = split_square(&snapshot, &built, k).unwrap();
        let used = reads() - before;
        assert_eq!((changes.created.len(), after.ids::<Face>().len()), (6, 2 * k * k + 4 * k + 1));
        used
    };
    let (small, large) = (read(22), read(70));
    assert!(small > 0);
    assert_eq!(small, large, "records read splitting one face of 1,056 faces, and of 10,080");
}
