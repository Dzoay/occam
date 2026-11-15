//! Source: original
//! Which way faces face (PARITY 1.10, orientation): each loop winds about its witness the way its face faces, so a face
//! agrees with each neighbour across their shared edge, and a solid faces out. Each corruption is caught by a commit and
//! by the whole-model check alike, naming its entities.

use std::sync::Arc;

use occam_geom::{Line, Plane};
use occam_report::{EntityKind, Reason};
use occam_units::{Direction, Point, UvPoint, Vec3};

use super::tests::{Failure, change, clean, failures, seeded, whole};
use crate::edit::Edit;
use crate::kind::Id;
use crate::models::{Built, Spec, build, cube, cylinder, facing, holed_sheet, polyhedron, sheet};
use crate::records::{
    BodyKind, BodyRecord, CurveRecord, EdgeRecord, FaceRecord, ShellRecord, SurfaceRecord, VertexRecord,
};
use crate::snapshot::Snapshot;
use crate::testing::{serial, tolerance, turn_inside_out, turn_loop, turn_over};

/// The failure of each loop of `built` in `loops`, against its face.
fn against(built: &Built, faces: &[usize]) -> Vec<Failure> {
    let mut found: Vec<Failure> = faces
        .iter()
        .map(|&k| (Reason::LoopAgainstFace, vec![built.loops[k].entity(), built.faces[k].entity()]))
        .collect();
    found.sort();
    found
}

#[test]
fn a_face_turned_over_alone_winds_against_its_loop() {
    // The review's flip1: one face of a box turned over, which measured its volume 4 where 6 was right.
    let found = seeded(&cube(), |edit, built| turn_over(edit, built.faces[0]));
    let (_, built) = clean(&cube());
    assert_eq!(found, (against(&built, &[0]), against(&built, &[0])));
    let found = seeded(&sheet(), |edit, built| turn_over(edit, built.faces[0]));
    let (_, built) = clean(&sheet());
    assert_eq!(found, (against(&built, &[0]), against(&built, &[0])), "a sheet's face too");
}

#[test]
fn every_face_turned_over_alone_is_named() {
    // The review's flip: every face of a box turned over, which measured its volume -6.
    let found = seeded(&cube(), |edit, built| built.faces.iter().for_each(|&f| turn_over(edit, f)));
    let (_, built) = clean(&cube());
    let all = against(&built, &[0, 1, 2, 3, 4, 5]);
    assert_eq!(found, (all.clone(), all));
}

#[test]
fn a_hole_running_the_way_its_outer_loop_runs_winds_against_its_face() {
    let found = seeded(&holed_sheet(), |edit, built| turn_loop(edit, built.loops[1]));
    let (_, built) = clean(&holed_sheet());
    let want = vec![(Reason::LoopAgainstFace, vec![built.loops[1].entity(), built.faces[0].entity()])];
    assert_eq!(found, (want.clone(), want));
    let found = seeded(&holed_sheet(), |edit, built| turn_over(edit, built.faces[0]));
    let mut both = vec![
        (Reason::LoopAgainstFace, vec![built.loops[0].entity(), built.faces[0].entity()]),
        (Reason::LoopAgainstFace, vec![built.loops[1].entity(), built.faces[0].entity()]),
    ];
    both.sort();
    assert_eq!(found, (both.clone(), both), "turned over, its outer loop and its hole both wind against it");
    let (snapshot, built) = clean(&holed_sheet());
    let mut edit = snapshot.edit();
    turn_inside_out(&mut edit, built.faces[0]);
    let next = edit.next.clone();
    assert!(edit.commit(&serial()).is_ok(), "turned over with both its loops, a sheet is as sound the other way up");
    assert_eq!(failures(&whole(&next)), []);
}

#[test]
fn a_face_turned_inside_out_whole_agrees_with_its_loops_and_not_with_its_neighbours() {
    let mut expected = Vec::new();
    let found = seeded(&cube(), |edit, built| {
        turn_inside_out(edit, built.faces[0]);
        for e in edit.view(built.faces[0]).unwrap().edges() {
            let named = [e.entity()].into_iter().chain(edit.view(e).unwrap().uses().into_iter().map(Id::entity));
            expected.push((Reason::UsesNotOpposed, named.collect()));
        }
        expected.sort();
    });
    assert_eq!(found, (expected.clone(), expected));
}

/// The report of a solid facing in: its shell, and the kind of entity where it reaches furthest.
fn inside_out(found: &[Failure]) -> Vec<(EntityKind, EntityKind)> {
    found
        .iter()
        .filter(|(reason, _)| *reason == Reason::ShellInsideOut)
        .map(|(_, named)| (named[0].kind, named[1].kind))
        .collect()
}

#[test]
fn a_cube_turned_inside_out_whole_faces_in() {
    // Every face turned over with its loops: each agrees with its loops and its neighbours, and the cube faces in.
    let (commit, whole) = seeded(&cube(), |edit, built| built.faces.iter().for_each(|&f| turn_inside_out(edit, f)));
    assert_eq!(commit, whole);
    assert_eq!(commit.len(), 1, "{commit:?}");
    assert_eq!(inside_out(&commit), [(EntityKind::Shell, EntityKind::Vertex)], "{commit:?}");
    let (_, built) = clean(&cube());
    assert_eq!(commit[0].1[0], built.shell.unwrap().entity());
}

#[test]
fn a_cylinder_turned_inside_out_faces_in() {
    // Along x it reaches furthest along the line its two patches meet on, from vertex to vertex.
    let (commit, whole) = seeded(&cylinder(), |edit, built| built.faces.iter().for_each(|&f| turn_inside_out(edit, f)));
    assert_eq!(commit, whole);
    assert_eq!(inside_out(&commit), [(EntityKind::Shell, EntityKind::Vertex)], "{commit:?}");
    assert_eq!(commit.len(), 1);
}

/// A unit cube whose faces each run clockwise seen from outside, its corner 0 at `x`: inside out, but sound otherwise.
fn inverted(kind: BodyKind, x: f64) -> Spec {
    let corners: Vec<[f64; 3]> = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
    ]
    .iter()
    .map(|&[a, b, c]| [a + x, b, c])
    .collect();
    let faces: [&[usize]; 6] =
        [&[1, 2, 3, 0], &[7, 6, 5, 4], &[4, 5, 1, 0], &[6, 7, 3, 2], &[3, 7, 4, 0], &[5, 6, 2, 1]];
    polyhedron(kind, &corners, &faces)
}

#[test]
fn a_solid_built_inside_out_is_refused() {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, &inverted(BodyKind::Solid, 0.0));
    let next = edit.next.clone();
    let commit = failures(&edit.commit(&serial()).unwrap_err());
    assert_eq!(commit, failures(&whole(&next)));
    assert_eq!(inside_out(&commit), [(EntityKind::Shell, EntityKind::Vertex)], "{commit:?}");
    assert_eq!((commit.len(), commit[0].1[0]), (1, built.shell.unwrap().entity()));
}

#[test]
fn a_closed_sheet_made_a_solid_is_asked_whether_it_faces_out() {
    let (snapshot, built) = clean(&inverted(BodyKind::Sheet, 0.0));
    assert_eq!(failures(&whole(&snapshot.model)), [], "a sheet may face either way");
    let mut edit = snapshot.edit();
    change(&mut edit, built.body, |r: &mut BodyRecord| r.kind = BodyKind::Solid);
    let next = edit.next.clone();
    let commit = failures(&edit.commit(&serial()).unwrap_err());
    assert_eq!(commit, failures(&whole(&next)));
    assert_eq!(inside_out(&commit), [(EntityKind::Shell, EntityKind::Vertex)], "{commit:?}");
}

#[test]
fn a_closed_sheet_that_faces_out_made_a_solid_passes() {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let spec = cube();
    let built = build(&mut edit, &Spec { kind: BodyKind::Sheet, ..spec });
    let snapshot = edit.commit(&serial()).unwrap().0;
    let mut edit = snapshot.edit();
    change(&mut edit, built.body, |r: &mut BodyRecord| r.kind = BodyKind::Solid);
    assert!(edit.commit(&serial()).is_ok());
}

#[test]
fn a_shell_moved_to_a_solid_is_asked_whether_it_faces_out() {
    // A cube, and beside it a closed sheet facing in: the sheet's shell moved to the cube's body reaches furthest. Moved
    // after the cube's shell, that shell changes too; moved first, the cube's body changes and its shell does not.
    let mut edit = Snapshot::empty(tolerance()).edit();
    let solid = build(&mut edit, &cube());
    let sheet = build(&mut edit, &inverted(BodyKind::Sheet, 4.0));
    let snapshot = edit.commit(&serial()).unwrap().0;
    let moved = |into: &Built, from: &Built, first: bool| {
        let mut edit = snapshot.edit();
        let (kept, shell) = (into.shell.unwrap(), from.shell.unwrap());
        if first {
            change(&mut edit, into.body, |r: &mut BodyRecord| r.first_shell = Some(shell));
            change(&mut edit, shell, |r: &mut ShellRecord| r.next = Some(kept));
        } else {
            change(&mut edit, kept, |r: &mut ShellRecord| r.next = Some(shell));
        }
        change(&mut edit, shell, |r: &mut ShellRecord| r.body = into.body);
        edit.remove(from.body).unwrap();
        edit
    };
    for first in [false, true] {
        let edit = moved(&solid, &sheet, first);
        let next = edit.next.clone();
        let commit = failures(&edit.commit(&serial()).unwrap_err());
        assert_eq!(commit, failures(&whole(&next)), "first: {first}");
        assert_eq!(inside_out(&commit), [(EntityKind::Shell, EntityKind::Vertex)], "first: {first}: {commit:?}");
        assert_eq!(commit[0].1[0], sheet.shell.unwrap().entity(), "the shell that reaches furthest");
    }
}

#[test]
fn a_solid_whose_every_face_is_written_again_as_it_was_is_asked_and_passes() {
    let (snapshot, built) = clean(&cube());
    let mut edit = snapshot.edit();
    for &f in &built.faces {
        change(&mut edit, f, |_: &mut FaceRecord| {});
    }
    assert!(edit.commit(&serial()).is_ok());
}

/// `snapshot`'s cube, `built`, mirrored in `edit` across the plane `x = 1/2`, onto itself: each vertex moved to its
/// image, each edge on its curve's image and each face on a plane through its centre's image, facing as the image of
/// its loop runs, which a mirror turns round. `in_place`, each curve and surface is replaced where it is; otherwise each
/// edge and face is given a new one.
fn mirrored(snapshot: &Snapshot, built: &Built, in_place: bool) -> Edit {
    let image = |p: Vec3| Vec3::new(1.0 - p.x, p.y, p.z);
    let turned = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
    let mut edit = snapshot.edit();
    for &v in &built.vertices {
        change(&mut edit, v, |r: &mut VertexRecord| r.point = Point::from_vec3(image(r.point.to_vec3())));
    }
    for &e in &built.edges {
        let record = edit.record(e).unwrap();
        let curve = edit.record(record.curve).unwrap().0;
        let (start, along) = curve.first(0.0);
        let line = Line::new(Point::from_vec3(image(start.to_vec3())), Direction::normalised(turned(along.rate())));
        if in_place {
            edit.set(record.curve, CurveRecord(Arc::new(line))).unwrap();
        } else {
            let new = edit.add(CurveRecord(Arc::new(line)));
            edit.set(e, EdgeRecord { curve: new, ..record }).unwrap();
        }
    }
    for &f in &built.faces {
        let record = edit.record(f).unwrap();
        let outer = edit.view(f).unwrap().loops()[0];
        let starts = edit.view(outer).unwrap().uses().into_iter().map(|u| edit.view(u).unwrap().vertices().unwrap().0);
        let points: Vec<Vec3> = starts.map(|v| edit.record(v).unwrap().point.to_vec3()).collect();
        let centre = points.iter().fold(Vec3::new(0.0, 0.0, 0.0), |c, &p| c + p) * 0.25;
        let surface = edit.record(record.surface).unwrap().0;
        let normal = surface.first(UvPoint::new(0.0, 0.0));
        // Along an axis, as `facing` takes it, without the roundings a turned frame leaves.
        let normal = normal.du.rate().cross(normal.dv.rate());
        let normal = Vec3::new(normal.x.round(), normal.y.round(), normal.z.round());
        let plane = SurfaceRecord(Arc::new(Plane::placed(&facing(turned(normal) * -1.0, centre))));
        if in_place {
            edit.set(record.surface, plane).unwrap();
        } else {
            let new = edit.add(plane);
            edit.set(f, FaceRecord { surface: new, ..record }).unwrap();
        }
    }
    edit
}

#[test]
fn a_cube_mirrored_onto_new_curves_and_surfaces_is_inside_out() {
    for place in [false, true] {
        let (snapshot, built) = clean(&cube());
        let mut edit = mirrored(&snapshot, &built, place);
        let _ = crate::geometric::measure(&mut edit);
        let next = edit.next.clone();
        let commit = failures(&edit.commit(&serial()).unwrap_err());
        assert_eq!(commit, failures(&whole(&next)), "in place: {place}");
        assert_eq!(inside_out(&commit), [(EntityKind::Shell, EntityKind::Vertex)], "in place: {place}: {commit:?}");
        assert_eq!(commit.len(), 1);
    }
}
