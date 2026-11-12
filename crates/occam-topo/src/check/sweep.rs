//! Source: original
//! The checker's promise, held over every way of breaking one record of each clean model: a commit reaches the same
//! failures from what the edit changed as the whole-model checks find in the model it would have made. Each corruption
//! replaces one field of one record with each other value of its kind, or removes one record, or replaces a geometry
//! in place with another of the model's, so that every path by which a commit widens what it checks is needed by some
//! corruption, or by none.

use occam_report::{Reason, StepRole};

use super::tests::{Failure, change, clean, failures, seeded, whole};
use crate::edit::Edit;
use crate::kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Shell, SurfaceGeom, Vertex, Wire,
};
use crate::models::{Spec, cube, cylinder, frame, sheet, wire};
use crate::records::{
    Around, BodyKind, BodyRecord, EdgeRecord, EdgeUseRecord, End, FaceRecord, LoopRecord, ShellRecord, UseOwner,
    VertexRecord, WireRecord,
};
use crate::snapshot::{Model, Snapshot};
use crate::testing::serial;

/// One way of breaking a model: what it is, and the edit that makes it.
type Corruption = (String, Box<dyn Fn(&mut Edit)>);

/// At most three of `list` to put in the place of the `i`th of something: the `i`th, the next, and the one halfway
/// round, so that each field of each record is tried against itself, a neighbour and a stranger.
fn picks<T: Copy + PartialEq>(list: &[T], i: usize) -> Vec<T> {
    let n = list.len();
    let mut out: Vec<T> = Vec::new();
    for k in [0, 1, n / 2] {
        if let Some(&x) = list.get(i.checked_add(k).map_or(0, |j| j.checked_rem(n).unwrap_or(0)))
            && !out.contains(&x)
        {
            out.push(x);
        }
    }
    out
}

/// The live record of `id` in `model`.
fn record_of<K: Kind>(model: &Model, id: Id<K>) -> K::Record {
    K::table(model).live(id.slot(), id.generation()).cloned().unwrap()
}

/// Every live entity of kind `K` in `model`.
fn all<K: Kind>(model: &Model) -> Vec<Id<K>> {
    K::table(model).records().map(|(slot, generation, _)| Id::new(slot, generation)).collect()
}

/// `id`'s record replaced through `how`, as a corruption named `what`.
fn set<R: crate::kind::Record + 'static>(what: String, id: Id<R::Kind>, how: impl Fn(&mut R) + 'static) -> Corruption {
    (what, Box::new(move |edit: &mut Edit| change(edit, id, |r: &mut R| how(r))))
}

/// The live ids of every kind in `model`.
struct Ids {
    bodies: Vec<Id<Body>>,
    shells: Vec<Id<Shell>>,
    faces: Vec<Id<Face>>,
    loops: Vec<Id<Loop>>,
    wires: Vec<Id<Wire>>,
    uses: Vec<Id<EdgeUse>>,
    edges: Vec<Id<Edge>>,
    vertices: Vec<Id<Vertex>>,
    surfaces: Vec<Id<SurfaceGeom>>,
    curves: Vec<Id<CurveGeom>>,
    pcurves: Vec<Id<ParamCurveGeom>>,
}

impl Ids {
    fn of(m: &Model) -> Self {
        Self {
            bodies: all(m),
            shells: all(m),
            faces: all(m),
            loops: all(m),
            wires: all(m),
            uses: all(m),
            edges: all(m),
            vertices: all(m),
            surfaces: all(m),
            curves: all(m),
            pcurves: all(m),
        }
    }
}

/// The corruptions of bodies, shells and faces.
fn of_bodies_shells_faces(ids: &Ids, out: &mut Vec<Corruption>) {
    for (i, &b) in ids.bodies.iter().enumerate() {
        for kind in [BodyKind::Solid, BodyKind::Sheet, BodyKind::Wire] {
            out.push(set(format!("{b:?} kind {kind:?}"), b, move |r: &mut BodyRecord| r.kind = kind));
        }
        out.push(set(format!("{b:?} no shell"), b, |r: &mut BodyRecord| r.first_shell = None));
        out.push(set(format!("{b:?} no wire"), b, |r: &mut BodyRecord| r.first_wire = None));
        for &s in &picks(&ids.shells, i) {
            out.push(set(format!("{b:?} shell {s:?}"), b, move |r: &mut BodyRecord| r.first_shell = Some(s)));
        }
        for &w in &picks(&ids.wires, i) {
            out.push(set(format!("{b:?} wire {w:?}"), b, move |r: &mut BodyRecord| r.first_wire = Some(w)));
        }
    }
    for (i, &s) in ids.shells.iter().enumerate() {
        out.push(set(format!("{s:?} next itself"), s, move |r: &mut ShellRecord| r.next = Some(s)));
        for &f in &picks(&ids.faces, i) {
            out.push(set(format!("{s:?} face {f:?}"), s, move |r: &mut ShellRecord| r.face = f));
        }
        let moved = move |edit: &mut Edit| {
            let born = edit.provenance(StepRole::Made);
            let body = edit.add(BodyRecord { kind: BodyKind::Solid, first_shell: Some(s), first_wire: None, born });
            change(edit, s, |r: &mut ShellRecord| r.body = body);
        };
        out.push((format!("{s:?} moved to a body of its own"), Box::new(moved)));
    }
    for (i, &f) in ids.faces.iter().enumerate() {
        out.push(set(format!("{f:?} turned"), f, |r: &mut FaceRecord| r.reversed = !r.reversed));
        for &l in &picks(&ids.loops, i) {
            out.push(set(format!("{f:?} outer {l:?}"), f, move |r: &mut FaceRecord| r.outer = l));
        }
        for &g in &picks(&ids.surfaces, i) {
            out.push(set(format!("{f:?} surface {g:?}"), f, move |r: &mut FaceRecord| r.surface = g));
        }
        let moved = move |edit: &mut Edit| {
            let body = edit.reserve::<Body>();
            let shell = edit.add(ShellRecord { body, face: f, next: None });
            let born = edit.provenance(StepRole::Made);
            edit.set(body, BodyRecord { kind: BodyKind::Sheet, first_shell: Some(shell), first_wire: None, born })
                .unwrap();
            change(edit, f, |r: &mut FaceRecord| r.shell = shell);
        };
        out.push((format!("{f:?} moved to a shell of its own"), Box::new(moved)));
    }
}

/// The corruptions of loops and wires.
fn of_loops_wires(ids: &Ids, out: &mut Vec<Corruption>) {
    for (i, &l) in ids.loops.iter().enumerate() {
        out.push(set(format!("{l:?} next none"), l, |r: &mut LoopRecord| r.next = None));
        for &f in &picks(&ids.faces, i) {
            out.push(set(format!("{l:?} face {f:?}"), l, move |r: &mut LoopRecord| r.face = f));
        }
        for &u in &picks(&ids.uses, i) {
            out.push(set(format!("{l:?} first {u:?}"), l, move |r: &mut LoopRecord| r.first = u));
        }
        for &n in &picks(&ids.loops, i) {
            out.push(set(format!("{l:?} next {n:?}"), l, move |r: &mut LoopRecord| r.next = Some(n)));
        }
    }
    for (i, &w) in ids.wires.iter().enumerate() {
        out.push(set(format!("{w:?} next itself"), w, move |r: &mut WireRecord| r.next = Some(w)));
        for &u in &picks(&ids.uses, i) {
            out.push(set(format!("{w:?} first {u:?}"), w, move |r: &mut WireRecord| r.first = u));
        }
    }
}

/// The corruptions of edge uses.
fn of_uses(ids: &Ids, out: &mut Vec<Corruption>) {
    for (i, &u) in ids.uses.iter().enumerate() {
        out.push(set(format!("{u:?} turned"), u, |r: &mut EdgeUseRecord| r.reversed = !r.reversed));
        out.push(set(format!("{u:?} next none"), u, |r: &mut EdgeUseRecord| r.next = None));
        out.push(set(format!("{u:?} no pcurve"), u, |r: &mut EdgeUseRecord| r.pcurve = None));
        for &e in &picks(&ids.edges, i) {
            out.push(set(format!("{u:?} edge {e:?}"), u, move |r: &mut EdgeUseRecord| r.edge = e));
        }
        for &l in &picks(&ids.loops, i) {
            out.push(set(format!("{u:?} loop {l:?}"), u, move |r: &mut EdgeUseRecord| r.owner = UseOwner::Loop(l)));
        }
        for &w in &picks(&ids.wires, i) {
            out.push(set(format!("{u:?} wire {w:?}"), u, move |r: &mut EdgeUseRecord| r.owner = UseOwner::Wire(w)));
        }
        for &n in &picks(&ids.uses, i) {
            out.push(set(format!("{u:?} next {n:?}"), u, move |r: &mut EdgeUseRecord| r.next = Some(n)));
            out.push(set(format!("{u:?} radial {n:?}"), u, move |r: &mut EdgeUseRecord| r.radial = n));
        }
        for &p in &picks(&ids.pcurves, i) {
            out.push(set(format!("{u:?} pcurve {p:?}"), u, move |r: &mut EdgeUseRecord| r.pcurve = Some(p)));
        }
    }
}

/// The corruptions of edges and vertices.
fn of_edges_vertices(ids: &Ids, out: &mut Vec<Corruption>) {
    for (i, &e) in ids.edges.iter().enumerate() {
        out.push(set(format!("{e:?} no uses"), e, |r: &mut EdgeRecord| r.first_use = None));
        for &c in &picks(&ids.curves, i) {
            out.push(set(format!("{e:?} curve {c:?}"), e, move |r: &mut EdgeRecord| r.curve = c));
        }
        for &v in &picks(&ids.vertices, i) {
            out.push(set(format!("{e:?} start {v:?}"), e, move |r: &mut EdgeRecord| r.ends.0 = v));
            out.push(set(format!("{e:?} finish {v:?}"), e, move |r: &mut EdgeRecord| r.ends.1 = v));
        }
        for &n in &picks(&ids.edges, i) {
            for end in [End::Start, End::Finish] {
                let around = Around { edge: n, end };
                out.push(set(format!("{e:?} around start {around:?}"), e, move |r: &mut EdgeRecord| {
                    r.around.0 = around;
                }));
                out.push(set(format!("{e:?} around finish {around:?}"), e, move |r: &mut EdgeRecord| {
                    r.around.1 = around;
                }));
            }
        }
        for &u in &picks(&ids.uses, i) {
            out.push(set(format!("{e:?} first use {u:?}"), e, move |r: &mut EdgeRecord| r.first_use = Some(u)));
        }
    }
    for (i, &v) in ids.vertices.iter().enumerate() {
        out.push(set(format!("{v:?} no star"), v, |r: &mut VertexRecord| r.star = None));
        for &e in &picks(&ids.edges, i) {
            for end in [End::Start, End::Finish] {
                let star = Some(Around { edge: e, end });
                out.push(set(format!("{v:?} star {star:?}"), v, move |r: &mut VertexRecord| r.star = star));
            }
        }
    }
}

/// Each record removed, and each geometry replaced in place by another of the model's.
fn removed_and_replaced(m: &Model, ids: &Ids, out: &mut Vec<Corruption>) {
    macro_rules! removed {
        ($($list:ident),*) => {
            $(for &id in &ids.$list {
                out.push((format!("{id:?} removed"), Box::new(move |edit: &mut Edit| edit.remove(id).unwrap())));
            })*
        };
    }
    removed!(bodies, shells, faces, loops, wires, uses, edges, vertices, surfaces, curves, pcurves);
    macro_rules! replaced {
        ($($list:ident),*) => {
            $(for (i, &id) in ids.$list.iter().enumerate() {
                for &other in &picks(&ids.$list, i) {
                    let record = record_of(m, other);
                    out.push((
                        format!("{id:?} replaced by {other:?}"),
                        Box::new(move |edit: &mut Edit| edit.set(id, record.clone()).unwrap()),
                    ));
                }
            })*
        };
    }
    replaced!(surfaces, curves, pcurves);
}

/// Every corruption of `snapshot`'s model.
fn corruptions(snapshot: &Snapshot) -> Vec<Corruption> {
    let ids = Ids::of(&snapshot.model);
    let mut out = Vec::new();
    of_bodies_shells_faces(&ids, &mut out);
    of_loops_wires(&ids, &mut out);
    of_uses(&ids, &mut out);
    of_edges_vertices(&ids, &mut out);
    removed_and_replaced(&snapshot.model, &ids, &mut out);
    out
}

/// Whether `reason` is about a shell as a whole: how its faces join, and what its counts give.
fn of_a_shell(reason: Reason) -> bool {
    matches!(reason, Reason::ShellNotConnected | Reason::GenusNotWhole)
}

/// Whether what a commit reports, `committed`, agrees with what the whole-model checks find, `found`: a commit refuses
/// exactly when they find anything; every failure but a shell's is found by both; and where a shell's joining or count
/// is all that is wrong, both name it alike. Where a list or a link is broken too, the joining and count that follow
/// from it may be named by one of them alone, and with another face where a search began elsewhere.
fn agree(committed: &[Failure], found: &[Failure]) -> bool {
    let rest = |all: &[Failure]| all.iter().filter(|(r, _)| !of_a_shell(*r)).cloned().collect::<Vec<_>>();
    let (c, f) = (rest(committed), rest(found));
    committed.is_empty() == found.is_empty() && c == f && (!f.is_empty() || committed == found)
}

/// What a commit of every `every`th corruption of `spec` reports, against what the whole-model checks find: how many
/// corruptions there are, and those where they do not agree.
fn disagreements(spec: &Spec, every: usize) -> (usize, Vec<String>) {
    let (snapshot, _) = clean(spec);
    let all = corruptions(&snapshot);
    let mut differ = Vec::new();
    for (what, corrupt) in all.iter().step_by(every) {
        let mut edit = snapshot.edit();
        corrupt(&mut edit);
        let _ = crate::geometric::measure(&mut edit);
        let next = edit.next.clone();
        let committed = edit.commit(&serial()).map_or_else(|reports| failures(&reports), |_| Vec::new());
        let found = failures(&whole(&next));
        if !agree(&committed, &found) {
            differ.push(format!("{what}: commit {committed:?}; whole {found:?}"));
        }
    }
    (all.len(), differ)
}

/// Asserts that a commit and the whole-model checks agree on every `every`th corruption of `spec`.
fn sweep(spec: &Spec, every: usize) {
    let (count, differ) = disagreements(spec, every);
    assert!(count > 100, "{count} corruptions");
    assert!(differ.is_empty(), "{} of {count} differ:\n{}", differ.len(), differ.join("\n"));
}

#[test]
fn a_commit_finds_what_the_whole_model_checks_find_however_one_record_of_a_cube_is_broken() {
    sweep(&cube(), 1);
}

#[test]
fn and_of_a_block_with_a_hole_through_it() {
    // Every third, as the block is four times the cube and each of its commits reads more.
    sweep(&frame(), 3);
}

#[test]
fn and_of_a_cylinder_of_two_patches() {
    sweep(&cylinder(), 1);
}

#[test]
fn and_of_a_sheet_and_of_a_wire() {
    sweep(&sheet(), 1);
    sweep(&wire(), 1);
}

#[test]
fn a_loop_moved_to_another_face_or_removed_leaves_its_face_unjoined_and_both_name_it_alike() {
    // A loop's edges go where it goes: the face it leaves is joined to no other, which a commit finds by searching from
    // the loop's faces before and after, and names as the whole-model check does.
    let moved =
        seeded(&cube(), |edit, built| change(edit, built.loops[2], |r: &mut LoopRecord| r.face = built.faces[3]));
    let removed = seeded(&cube(), |edit, built| edit.remove(built.loops[2]).unwrap());
    for (committed, found) in [moved, removed] {
        assert!(committed.iter().any(|(r, _)| *r == Reason::ShellNotConnected), "{committed:?}");
        assert_eq!(committed, found);
    }
}
