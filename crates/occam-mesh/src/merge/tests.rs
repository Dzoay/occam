//! Source: original

use std::num::NonZeroUsize;

use occam_report::{Code, Fault, Reason};
use occam_units::{Angle, Length, Vec3};

use super::*;
use crate::faces::FaceMesh;
use crate::testing::{collapsed_patch, disc, disc_drawn, model, plane_and_cylinder, sphere_half};

fn limits(deflection: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap()
}

/// `range` as indices.
fn indices(range: &Range<u32>) -> Range<usize> {
    usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()
}

/// What `block` holds of `merged`, numbered from its first vertex, as a face's own mesh.
fn own(merged: &Mesh, block: &Block) -> FaceMesh {
    let first = block.vertices.start;
    let back = |k: u32| k.checked_sub(first).unwrap();
    let sides = block
        .sides
        .iter()
        .map(|side| Side { vertices: side.vertices.iter().map(|&k| back(k)).collect(), ..side.clone() });
    FaceMesh {
        points: merged.points[indices(&block.vertices)].to_vec(),
        normals: merged.normals[indices(&block.vertices)].to_vec(),
        uvs: merged.uvs[indices(&block.vertices)].to_vec(),
        triangles: merged.triangles[indices(&block.triangles)].iter().map(|t| t.map(back)).collect(),
        sides: sides.collect(),
        capped: block.capped,
    }
}

/// A face of a model, to be named.
fn a_face() -> Id<Face> {
    model(&[disc_drawn(1.0)]).0.ids::<Face>()[0]
}

#[test]
fn the_faces_merge_by_index_each_a_block_of_its_own_in_id_order() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), disc(1.0), collapsed_patch()]);
    let edges = sample_edges(&Exec::serial(), &snapshot, limits(0.05));
    let faces = mesh_faces(&Exec::serial(), &snapshot, &edges);
    let merged = merge(edges.clone(), &faces);
    assert_eq!(merged.edges, edges);
    assert_eq!(merged.refused, faces.refused().cloned().collect::<Vec<_>>());
    assert_eq!(merged.refused.len(), 1, "the disc, whose circle is not drawn in it");
    assert!(merged.open.is_empty(), "merging checks nothing");
    let meshed: Vec<(Id<Face>, &FaceMesh)> = faces.iter().filter_map(|(f, m)| m.ok().map(|m| (f, m))).collect();
    assert_eq!(merged.blocks.len(), meshed.len());
    let (mut vertices, mut triangles) = (0, 0);
    for (block, (face, mesh)) in merged.blocks.iter().zip(&meshed) {
        assert_eq!(block.face, *face);
        assert_eq!(merged.block(*face), Some(block));
        assert_eq!(block.vertices.start, vertices, "each block follows the last");
        assert_eq!(block.triangles.start, triangles);
        assert_eq!(own(&merged, block), **mesh);
        vertices = block.vertices.end;
        triangles = block.triangles.end;
    }
    let ends = [merged.points.len(), merged.normals.len(), merged.uvs.len(), merged.triangles.len()];
    assert_eq!(ends.map(|n| u32::try_from(n).unwrap()), [vertices, vertices, vertices, triangles]);
    assert_eq!(merged.block(snapshot.ids::<Face>()[2]), None, "the disc has no block");
}

#[test]
fn a_vertex_is_numbered_in_the_mesh_from_its_blocks_first_and_never_past_u32_max() {
    assert_eq!(offset(5, 7), Some(12));
    assert_eq!(offset(u32::MAX - 1, 1), Some(u32::MAX));
    assert_eq!(offset(u32::MAX, 1), None);
    assert_eq!(numbered(7, 0), Some(7..7));
    assert_eq!(numbered(u32::MAX - 2, 2), Some(u32::MAX - 2..u32::MAX));
    assert_eq!(numbered(u32::MAX - 2, 3), None);
}

#[test]
fn a_face_that_would_number_past_u32_max_is_refused_with_its_budget_spent() {
    let face = a_face();
    let report = too_many(face);
    assert_eq!(
        (report.code, report.fault, report.reason),
        (Code::FaceNotMeshed, Some(Fault::Internal), Some(Reason::BudgetSpent))
    );
    assert_eq!(report.entities, [face.entity()]);
    let (snapshot, _) = model(&[disc_drawn(1.0)]);
    let edges = sample_edges(&Exec::serial(), &snapshot, limits(0.1));
    let own = crate::faces::mesh_face(&snapshot, face, &edges).unwrap();
    let most = usize::try_from(u32::MAX).unwrap();
    assert!(placed(face, &own, (0, 0)).is_some());
    let last = most - own.points.len();
    assert_eq!(placed(face, &own, (last, 0)).map(|block| block.vertices.end), Some(u32::MAX), "the last that fits");
    assert_eq!(placed(face, &own, (last + 1, 0)), None, "a vertex past u32::MAX");
    assert_eq!(placed(face, &own, (0, most - own.triangles.len() + 1)), None, "a triangle past it");
    // On a 32-bit target (wasm32) no vertex count can pass u32::MAX, so there is nothing further to refuse.
    if let Some(past) = most.checked_add(1) {
        assert_eq!(placed(face, &own, (past, 0)), None);
    }
}

#[test]
fn a_mesh_is_the_same_bits_and_hash_at_one_thread_and_at_four() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), collapsed_patch(), sphere_half(2.0), disc_drawn(1.0)]);
    let four = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let serial = mesh(&Exec::serial(), &snapshot, limits(0.05));
    assert_eq!(serial.blocks.len(), 5);
    let parallel = mesh(&four, &snapshot, limits(0.05));
    assert_eq!(serial, parallel);
    assert_eq!(serial.digest(), parallel.digest());
}

/// A change to a mesh.
type Change<'a> = Box<dyn Fn(&mut Mesh) + 'a>;

#[test]
fn the_hash_changes_with_one_ulp_one_corner_one_flag_or_one_report_and_not_with_a_nans_bits() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0)]);
    let whole = mesh(&Exec::serial(), &snapshot, limits(0.05));
    let up = |p: Point<World>| Point::from_vec3(Vec3::new(p.to_vec3().x.next_up(), p.to_vec3().y, p.to_vec3().z));
    let changes: Vec<Change> = vec![
        Box::new(|m| m.points[3] = up(m.points[3])),
        Box::new(|m| m.normals[3] = m.normals[3].reversed()),
        Box::new(|m| m.uvs[3] = UvPoint::new(m.uvs[3].u().next_up(), m.uvs[3].v())),
        Box::new(|m| m.triangles[0].swap(1, 2)),
        Box::new(|m| m.blocks[0].capped = true),
        Box::new(|m| m.blocks[1].sides[0].reversed ^= true),
        Box::new(|m| m.blocks[1].sides[0].vertices[1] += 1),
        Box::new(|m| m.blocks[0].vertices.end -= 1),
        Box::new(|m| m.blocks[0].triangles.end -= 1),
        Box::new(|m| m.blocks.swap(0, 1)),
        Box::new(|m| m.refused.push(too_many(m.blocks[0].face))),
        Box::new(|m| m.open.push(too_many(m.blocks[0].face))),
        Box::new(|m| m.edges = sample_edges(&Exec::serial(), &model(&[plane_and_cylinder(2.0)]).0, limits(0.005))),
    ];
    let mut seen = vec![whole.digest()];
    for change in &changes {
        let mut altered = whole.clone();
        change(&mut altered);
        seen.push(altered.digest());
    }
    let mut distinct = seen.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), seen.len(), "{seen:x?}");
    let nan = |bits: u64| {
        let mut m = whole.clone();
        m.points[0] = Point::from_vec3(Vec3::new(f64::from_bits(bits), 0.0, 0.0));
        m.digest()
    };
    assert_eq!(nan(0x7ff8_0000_0000_0001), nan(0xfff8_0000_0000_0000), "a NaN is hashed as one pattern");
}
