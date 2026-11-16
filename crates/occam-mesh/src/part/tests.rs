//! Source: original
#![expect(clippy::arithmetic_side_effects, reason = "small counts and offsets in tests")]

use std::num::NonZeroUsize;
use std::ops::Range;

use occam_report::{Code, EntityRef, Fault, Reason, Report};
use occam_units::{Angle, Length, Point, Vec3, World};

use super::*;
use crate::buffers::{Buffers, FaceRange};
use crate::merge::mesh;
use crate::testing::{collapsed_patch, disc_drawn, model, plane_and_cylinder, sphere_half, undrawn};

fn limits(deflection: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap()
}

/// `parts`, each numbered from its own start, laid end to end in the order given, as the page lays its workers' parts.
fn laid_end_to_end(parts: &[Buffers]) -> Buffers {
    let mut joined = Buffers { positions: vec![], normals: vec![], uvs: vec![], indices: vec![], faces: vec![] };
    for part in parts {
        let (vertices, triangles) =
            (u32::try_from(joined.vertices()).unwrap(), u32::try_from(joined.triangles()).unwrap());
        let moved = |r: &Range<u32>, by: u32| r.start + by..r.end + by;
        joined.faces.extend(part.faces.iter().map(|range| FaceRange {
            face: range.face,
            vertices: moved(&range.vertices, vertices),
            triangles: moved(&range.triangles, triangles),
        }));
        joined.positions.extend(&part.positions);
        joined.normals.extend(&part.normals);
        joined.uvs.extend(&part.uvs);
        joined.indices.extend(&part.indices);
    }
    joined
}

/// Every way of cutting `n` faces this test tries: each face alone, and two parts cut at each place.
fn cuts(n: usize) -> Vec<Vec<Range<usize>>> {
    let mut cuts: Vec<Vec<Range<usize>>> = vec![(0..n).map(|k| k..k + 1).collect()];
    cuts.extend((0..=n).map(|k| vec![0..k, k..n]));
    cuts
}

#[test]
fn a_models_faces_meshed_in_parts_and_laid_end_to_end_are_its_whole_meshs_buffers_at_one_thread_and_four() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), collapsed_patch(), sphere_half(2.0), disc_drawn(1.0)]);
    let faces = snapshot.ids::<Face>();
    let whole = mesh(&Exec::serial(), &snapshot, limits(0.05));
    assert_eq!((whole.blocks.len(), whole.open.clone()), (5, vec![]));
    let four = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    for exec in [Exec::serial(), four] {
        for cut in cuts(faces.len()) {
            let parts: Vec<Mesh> =
                cut.iter().map(|r| mesh_part(&exec, &snapshot, limits(0.05), &faces[r.clone()])).collect();
            let buffers: Vec<Buffers> = parts.iter().map(Buffers::of).collect();
            assert_eq!(laid_end_to_end(&buffers), Buffers::of(&whole), "cut {cut:?}");
            assert!(parts.iter().all(|part| part.refused.is_empty() && part.open.is_empty()), "cut {cut:?}");
        }
    }
}

#[test]
fn every_face_as_one_part_is_the_whole_mesh_refusals_holes_and_all() {
    let (snapshot, _) = model(&[undrawn(plane_and_cylinder(2.0), 0), sphere_half(2.0)]);
    let whole = mesh(&Exec::serial(), &snapshot, limits(0.05));
    assert_eq!((whole.refused.len(), whole.open.len()), (1, 1), "the plane, whose edges are not drawn in it");
    let part = mesh_part(&Exec::serial(), &snapshot, limits(0.05), &snapshot.ids::<Face>());
    assert_eq!(part, whole);
    assert_eq!(part.digest(), whole.digest());
}

/// What each report says: its code, its reason and the entities it names.
fn said(reports: &[Report]) -> Vec<(Code, Option<Reason>, Vec<EntityRef>)> {
    reports.iter().map(|report| (report.code, report.reason, report.entities.clone())).collect()
}

#[test]
fn a_part_is_checked_as_far_as_it_reaches_a_crack_inside_it_caught_and_its_cut_from_the_rest_not_open() {
    let (snapshot, edges) = model(&[plane_and_cylinder(2.0)]);
    let faces = snapshot.ids::<Face>();
    let alone = mesh_part(&Exec::serial(), &snapshot, limits(0.01), &faces[..1]);
    assert_eq!(alone.open, [], "the line it shares with the cylinder, outside the part, is not open");
    let shared = edges[0][3];
    let side = alone.blocks[0].sides.iter().find(|side| side.edge == shared).unwrap();
    let k = usize::try_from(side.vertices[side.vertices.len() / 2]).unwrap();
    let nudge = |p: Point<World>| Point::from_vec3(Vec3::new(p.to_vec3().x.next_up(), p.to_vec3().y, p.to_vec3().z));
    let mut both = mesh_part(&Exec::serial(), &snapshot, limits(0.01), &faces);
    both.points[k] = nudge(both.points[k]);
    let named = vec![shared.entity(), faces[0].entity(), faces[1].entity()];
    assert_eq!(
        said(&within(&snapshot, &both, &|_| true)),
        [(Code::NotWatertight, Some(Reason::SegmentsUnmatched), named)]
    );
    let mut cut = alone;
    cut.points[k] = nudge(cut.points[k]);
    assert_eq!(within(&snapshot, &cut, &|face| face == faces[0]), [], "the cylinder's side of it is in another part");
}

#[test]
fn a_face_refused_in_a_part_is_a_hole_there_and_no_hole_in_the_part_beside_it() {
    let (snapshot, _) = model(&[undrawn(plane_and_cylinder(2.0), 0)]);
    let faces = snapshot.ids::<Face>();
    let plane = mesh_part(&Exec::serial(), &snapshot, limits(0.05), &faces[..1]);
    let refused: Vec<(Code, EntityRef)> = plane.refused.iter().map(|r| (r.code, r.entities[0])).collect();
    assert_eq!(refused, [(Code::FaceNotMeshed, faces[0].entity())]);
    let hole = (Code::NotWatertight, Some(Reason::FaceMissing), vec![faces[0].entity()]);
    assert_eq!(said(&plane.open), [hole], "a hole with no neighbour in the part to be open along");
    assert_eq!(plane.open[0].fault, Some(Fault::NotImplemented), "the refusal's fault");
    let cylinder = mesh_part(&Exec::serial(), &snapshot, limits(0.05), &faces[1..]);
    assert_eq!((cylinder.refused.clone(), cylinder.open.clone()), (vec![], vec![]), "the plane is another part's");
    assert_eq!(cylinder.blocks.iter().map(|block| block.face).collect::<Vec<_>>(), [faces[1]]);
}
