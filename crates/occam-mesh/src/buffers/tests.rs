//! Source: original

use occam_math::Exec;
use occam_report::{EntityKind, EntityRef};
use occam_units::{Angle, Length};

use super::*;
use crate::limits::Limits;
use crate::merge::mesh;
use crate::testing::{collapsed_patch, disc_drawn, model, plane_and_cylinder, undrawn};

fn limits(deflection: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap()
}

/// `range` as indices.
fn indices(range: &Range<u32>) -> Range<usize> {
    usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()
}

/// The bits of `numbers`.
fn bits(numbers: &[f64]) -> Vec<u64> {
    numbers.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn a_mesh_becomes_flat_arrays_each_faces_indices_counted_from_its_block() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), undrawn(disc_drawn(1.0), 0), collapsed_patch()]);
    let made = mesh(&Exec::serial(), &snapshot, limits(0.05));
    assert_eq!(made.refused.len(), 1, "the disc, whose circle is not drawn in it");
    let buffers = Buffers::of(&made);
    let n = made.points.len();
    assert_eq!((buffers.vertices(), buffers.triangles()), (n, made.triangles.len()));
    assert_eq!((buffers.positions.len(), buffers.normals.len(), buffers.uvs.len()), (3 * n, 3 * n, 2 * n));
    let positions: Vec<f64> = made.points.iter().map(|p| p.to_vec3()).flat_map(|v| [v.x, v.y, v.z]).collect();
    let normals: Vec<f64> = made.normals.iter().map(|d| d.get()).flat_map(|v| [v.x, v.y, v.z]).collect();
    let uvs: Vec<f64> = made.uvs.iter().flat_map(|uv| [uv.u(), uv.v()]).collect();
    assert_eq!(bits(&buffers.positions), bits(&positions));
    assert_eq!(bits(&buffers.normals), bits(&normals));
    assert_eq!(bits(&buffers.uvs), bits(&uvs));
    assert_eq!(buffers.faces.len(), 3, "the plane, the cylinder and the patch");
    assert_eq!(buffers.indices.len(), 3 * made.triangles.len());
    for (range, block) in buffers.faces.iter().zip(&made.blocks) {
        assert_eq!((range.face, &range.vertices, &range.triangles), (block.face, &block.vertices, &block.triangles));
        assert_eq!(buffers.face(block.face), Some(range));
        let own = &buffers.indices[3 * indices(&range.triangles).start..3 * indices(&range.triangles).end];
        let merged: Vec<u32> = made.triangles[indices(&block.triangles)].iter().flatten().copied().collect();
        let back: Vec<u32> = merged.iter().map(|&k| k - block.vertices.start).collect();
        assert_eq!(own, back, "{:?}: counted from its block's first vertex", block.face);
        let count = block.vertices.end - block.vertices.start;
        assert!(own.iter().all(|&k| k < count), "{:?}: every index in its block", block.face);
    }
    let refused = Id::from_entity(made.refused[0].entities[0]).unwrap();
    assert_eq!(buffers.face(refused), None, "a face refused has no range");
}

#[test]
fn the_bytes_are_the_counts_then_each_array_little_endian() {
    let face = |slot| Id::from_entity(EntityRef::new(EntityKind::Face, slot, 2)).unwrap();
    let place = |k: u32| f64::from(k) * 0.5 - 3.0;
    let buffers = Buffers {
        positions: (0..21).map(place).collect(),
        normals: (21..42).map(place).collect(),
        uvs: (42..56).map(place).collect(),
        indices: vec![0, 1, 2, 0, 1, 3, 1, 2, 3],
        faces: vec![
            FaceRange { face: face(5), vertices: 0..3, triangles: 0..1 },
            FaceRange { face: face(9), vertices: 3..7, triangles: 1..3 },
        ],
    };
    let mut want: Vec<u8> = [7_u64, 3, 2].iter().flat_map(|n| n.to_le_bytes()).collect();
    want.extend((0..56).map(place).flat_map(f64::to_le_bytes));
    want.extend([0_u32, 1, 2, 0, 1, 3, 1, 2, 3].iter().flat_map(|k| k.to_le_bytes()));
    want.extend([5_u32, 2, 0, 3, 0, 1, 9, 2, 3, 4, 1, 2].iter().flat_map(|k| k.to_le_bytes()));
    let bytes = buffers.to_bytes();
    assert_eq!(bytes.len(), 24 + 8 * 56 + 4 * 9 + 4 * 12);
    assert_eq!(bytes, want);
}
