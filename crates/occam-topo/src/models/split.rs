//! Source: original
//! A top square of [`super::plate`] split by its diagonal into two triangles, in an edit: the checker's tests split one to
//! show that what a commit reads follows the edit, and the workloads split many to time edits as S3-P3 did.
#![expect(clippy::arithmetic_side_effects, reason = "small indices in test models")]

use core::f64::consts::SQRT_2;
use std::sync::Arc;

use occam_geom::Line;
use occam_report::StepRole;
use occam_units::{Direction, Length, UvPoint};

use super::{Built, join_star};
use crate::edit::Edit;
use crate::kind::{Edge, EdgeUse, Face, Id, Loop};
use crate::records::{Around, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord, UseOwner};

/// The top square `(i, j)` of a plate `k` squares across, as [`super::plate`] numbers its faces.
pub(crate) fn top_square(built: &Built, k: usize, (i, j): (usize, usize)) -> Id<Face> {
    built.faces[k * k + j * k + i]
}

/// The uses of `face`'s outer loop, from its first, in `edit`.
fn outer_uses(edit: &Edit, face: Id<Face>) -> Vec<Id<EdgeUse>> {
    let first = edit.record(edit.record(face).unwrap().outer).unwrap().first;
    let mut found = vec![first];
    while let Some(next) = edit.record(*found.last().unwrap()).unwrap().next.filter(|n| *n != first) {
        found.push(next);
    }
    found
}

/// Gives the use `id` its record changed by `how`, in `edit`.
fn change(edit: &mut Edit, id: Id<EdgeUse>, how: impl FnOnce(&mut EdgeUseRecord)) {
    let mut record = edit.record(id).unwrap();
    how(&mut record);
    edit.set(id, record).unwrap();
}

/// The vertices a use runs from and to, in `edit`.
fn ends(edit: &Edit, x: Id<EdgeUse>) -> (Id<crate::kind::Vertex>, Id<crate::kind::Vertex>) {
    let side = edit.record(x).unwrap();
    let edge = edit.record(side.edge).unwrap();
    if side.reversed { (edge.ends.1, edge.ends.0) } else { edge.ends }
}

/// Splits the unit square `face` of a plate, in `edit`, by the diagonal from the start of its first side to the end of
/// its second: the square keeps those two sides, and a new triangle, on the same surface and in the same shell, takes
/// the other two. Each keeps a witness at its centroid.
pub(crate) fn split(edit: &mut Edit, face: Id<Face>) {
    let (square, sides) = (edit.record(face).unwrap(), outer_uses(edit, face));
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
    change(edit, sides[1], |r| r.next = Some(da));
    change(edit, sides[2], |r| r.owner = UseOwner::Loop(other));
    change(edit, sides[3], |r| (r.owner, r.next) = (UseOwner::Loop(other), Some(db)));
    let mut outer = edit.record(square.outer).unwrap();
    outer.witness = UvPoint::new(1.0 / 6.0, -1.0 / 6.0);
    edit.set(square.outer, outer).unwrap();
    let witness = UvPoint::new(-1.0 / 6.0, 1.0 / 6.0);
    edit.set(other, LoopRecord { face: half, first: sides[2], next: None, witness }).unwrap();
    edit.set(half, FaceRecord { outer: other, born, ..square }).unwrap();
}
