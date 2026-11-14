//! Source: original
//! What a commit reads grows in step with what the edit touched, never with its square: a face of many edges checks each
//! use against its loop's list read once, not once a use, and an edit that takes many curves away, as an extrusion takes
//! its profile, finds what lies on them in one scan of the model, not one a curve.
#![expect(clippy::arithmetic_side_effects, reason = "small indices and counts in tests")]

use super::read::reads;
use super::tests::clean;
use crate::kind::{Body, CurveGeom, Edge, EdgeUse, Face, Loop, Shell, SurfaceGeom, Vertex};
use crate::models::{Built, Spec, build, polyhedron};
use crate::records::BodyKind;
use crate::snapshot::Snapshot;
use crate::testing::{serial, tolerance};

/// The corners of a staircase of `m` steps, anticlockwise in XY: from `(-m, -m)` to `(m, -m)`, up to the first step at
/// `(m, 1)`, up the steps to `(0, m)`, and round by `(-m, m)`. `2m + 3` corners, every side along an axis, as
/// [`polyhedron`] asks, and their mean clear inside.
fn staircase(m: usize) -> Vec<[f64; 3]> {
    let whole = |n: usize| f64::from(u32::try_from(n).unwrap());
    let mut corners = vec![[-whole(m), -whole(m), 0.0], [whole(m), -whole(m), 0.0]];
    for i in 1..=m {
        corners.push([whole(m - i + 1), whole(i), 0.0]);
        corners.push([whole(m - i), whole(i), 0.0]);
    }
    corners.push([-whole(m), whole(m), 0.0]);
    corners
}

/// The staircase of `m` steps as a sheet of one face.
fn sheet(m: usize) -> Spec {
    let corners = staircase(m);
    let face: Vec<usize> = (0..corners.len()).collect();
    polyhedron(BodyKind::Sheet, &corners, &[&face])
}

/// The staircase of `m` steps swept one unit up into a prism: its bottom, its top, then a side for each of its sides.
fn prism(m: usize) -> Spec {
    let ring = staircase(m);
    let n = ring.len();
    let corners: Vec<[f64; 3]> =
        [0.0, 1.0].iter().flat_map(|&z| ring.iter().map(move |&[x, y, _]| [x, y, z])).collect();
    let mut faces: Vec<Vec<usize>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
    faces.extend((0..n).map(|k| vec![k, (k + 1) % n, n + (k + 1) % n, n + k]));
    let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
    polyhedron(BodyKind::Solid, &corners, &faces)
}

/// Removes the sheet `built` in `edit`, as an extrusion takes its profile: every entity of it, and its curves and surface.
fn take(edit: &mut crate::edit::Edit, snapshot: &Snapshot, built: &Built) {
    for &e in &built.edges {
        edit.remove::<CurveGeom>(snapshot.view(e).unwrap().curve_id()).unwrap();
        edit.remove::<Edge>(e).unwrap();
    }
    for &f in &built.faces {
        edit.remove::<SurfaceGeom>(snapshot.view(f).unwrap().surface_id()).unwrap();
        edit.remove::<Face>(f).unwrap();
    }
    built.uses.iter().for_each(|&u| edit.remove::<EdgeUse>(u).unwrap());
    built.loops.iter().for_each(|&l| edit.remove::<Loop>(l).unwrap());
    built.vertices.iter().for_each(|&v| edit.remove::<Vertex>(v).unwrap());
    edit.remove::<Shell>(built.shell.unwrap()).unwrap();
    edit.remove::<Body>(built.body).unwrap();
}

/// Asserts that `read` at 8, 16 and 32 steps grows by the same amount for each step added.
fn in_step(read: impl Fn(usize) -> usize, what: &str) {
    let (one, two, four) = (read(8), read(16), read(32));
    assert!(one > 0);
    assert_eq!(four - two, 2 * (two - one), "records read {what}, at 8, 16 and 32 steps: {one}, {two}, {four}");
}

#[test]
fn what_a_commit_reads_making_a_prism_grows_in_step_with_its_sides() {
    in_step(
        |m| {
            let mut edit = Snapshot::empty(tolerance()).edit();
            build(&mut edit, &prism(m));
            let before = reads();
            edit.commit(&serial()).unwrap();
            reads() - before
        },
        "making a prism over a staircase",
    );
}

#[test]
fn what_a_commit_reads_sweeping_a_face_into_a_prism_grows_in_step_with_its_sides() {
    in_step(
        |m| {
            let (snapshot, built) = clean(&sheet(m));
            let mut edit = snapshot.edit();
            take(&mut edit, &snapshot, &built);
            build(&mut edit, &prism(m));
            let before = reads();
            edit.commit(&serial()).unwrap();
            reads() - before
        },
        "taking a staircase sheet and making its prism",
    );
}
