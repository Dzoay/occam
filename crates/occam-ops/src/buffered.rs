//! Source: original
//! Models `construction` makes, as the renderer's buffers (view.buffers): a cube's and a cylinder's, each face a vertex
//! block whose indices count from its start, every vertex on its face's surface at its place; and meshed again after a
//! step, only what the step touched, the rest kept bit for bit and the whole the mesh the model gives meshed anew.
//! `occam-mesh` sits above this crate in the layers, so its tests on built models live here.
#![expect(clippy::arithmetic_side_effects, reason = "small indices and counts in tests")]

use occam_math::Exec;
use occam_mesh::{Buffers, Limits, Mesh, remesh};
use occam_topo::{Changes, Edge, Face, Id, Snapshot};
use occam_units::{Angle, Length, UvPoint, Vec3};
use serde_json::{Value, json};

use crate::construct::tests::{built, request};
use crate::meshed::sector_with_a_hole;
use crate::{Labels, Request, execute};

/// `model` meshed whole at 0.1.
fn whole(model: &Snapshot) -> Mesh {
    let limits = Limits::new(Length::from_millimetres(0.1), Angle::from_radians(0.35)).unwrap();
    occam_mesh::mesh(&Exec::serial(), model, limits)
}

/// `model` after the steps `then`, each through an edit that `commit` checks, and what each step changed.
fn edited(model: &Snapshot, then: Value) -> (Snapshot, Vec<Changes>) {
    let request = Request::parse(&request(then).to_string()).unwrap();
    let (mut model, mut changes) = (model.clone(), Vec::new());
    for step in &request.steps {
        let mut edit = model.edit();
        execute(&mut edit, step, &Labels::default()).unwrap();
        let (next, step) = edit.commit(&Exec::serial()).unwrap();
        model = next;
        changes.push(step);
    }
    (model, changes)
}

/// What the buffers hold of `face`: its positions', normals' and places' bits and its indices.
fn block(buffers: &Buffers, face: Id<Face>) -> [Vec<u64>; 4] {
    let range = buffers.face(face).unwrap();
    let (first, last) = (range.vertices.start as usize, range.vertices.end as usize);
    let (start, end) = (range.triangles.start as usize, range.triangles.end as usize);
    let bits = |numbers: &[f64], each: usize| numbers[each * first..each * last].iter().map(|x| x.to_bits()).collect();
    let indices = buffers.indices[3 * start..3 * end].iter().map(|&k| u64::from(k)).collect();
    [bits(&buffers.positions, 3), bits(&buffers.normals, 3), bits(&buffers.uvs, 2), indices]
}

#[test]
fn a_cubes_and_a_cylinders_buffers_hold_each_vertex_on_its_face_at_its_place() {
    let cube = json!([{"op": "box", "size": [10, 10, 10]}]);
    let cylinder = json!([{"op": "cylinder", "radius": 4, "height": 10}]);
    // A cylinder's side is two faces, each half a turn, and its caps two more.
    for (name, steps, faces) in [("cube", cube, 6), ("cylinder", cylinder, 4)] {
        let (model, _) = built(steps);
        let made = whole(&model);
        assert_eq!((made.refused.len(), made.open.len()), (0, 0), "{name}");
        let buffers = Buffers::of(&made);
        let (n, t) = (buffers.vertices(), buffers.triangles());
        assert_eq!((buffers.positions.len(), buffers.normals.len(), buffers.uvs.len()), (3 * n, 3 * n, 2 * n));
        assert_eq!(buffers.indices.len(), 3 * t, "{name}");
        assert_eq!(buffers.faces.iter().map(|r| r.face).collect::<Vec<_>>(), model.ids::<Face>(), "{name}");
        assert_eq!(buffers.faces.len(), faces);
        let (mut vertices, mut triangles) = (0, 0);
        for range in &buffers.faces {
            assert_eq!((range.vertices.start, range.triangles.start), (vertices, triangles), "{name}: in order");
            (vertices, triangles) = (range.vertices.end, range.triangles.end);
            let count = range.vertices.end - range.vertices.start;
            let own = &buffers.indices[3 * range.triangles.start as usize..3 * range.triangles.end as usize];
            assert!(!own.is_empty() && own.iter().all(|&k| k < count), "{name}: {:?} indexes its block", range.face);
            on_its_surface(&model, &buffers, range.face);
        }
        assert_eq!((vertices as usize, triangles as usize), (n, t), "{name}: the ranges cover the arrays");
        if name == "cube" {
            assert_eq!((n, t), (24, 12), "each square its four corners and two triangles");
        }
    }
}

/// Each vertex of `face` in `buffers` within the band of its face's surface at its place, its normal of unit length.
fn on_its_surface(model: &Snapshot, buffers: &Buffers, face: Id<Face>) {
    let range = buffers.face(face).unwrap();
    let surface = model.view(face).unwrap().surface().unwrap();
    let band = model.tolerance().band().millimetres();
    for k in range.vertices.start as usize..range.vertices.end as usize {
        let at = UvPoint::new(buffers.uvs[2 * k], buffers.uvs[2 * k + 1]);
        let off = (triple(&buffers.positions, k) - surface.point(at).to_vec3()).norm();
        assert!(off <= band, "{face:?}: vertex {k} is {off} from its surface at its place");
        assert!((triple(&buffers.normals, k).norm() - 1.0).abs() < 1e-12, "{face:?}: vertex {k}'s normal");
    }
}

/// The three numbers of vertex `k` in `numbers`.
fn triple(numbers: &[f64], k: usize) -> Vec3 {
    Vec3::new(numbers[3 * k], numbers[3 * k + 1], numbers[3 * k + 2])
}

/// The faces of `model` that `before` does not hold.
fn faces_new(model: &Snapshot, before: &Snapshot) -> Vec<Id<Face>> {
    model.ids::<Face>().into_iter().filter(|f| before.view(*f).is_none()).collect()
}

/// The edges the faces `faces` of `model` use, in id order.
fn their_edges(model: &Snapshot, faces: &[Id<Face>]) -> Vec<Id<Edge>> {
    let mut edges: Vec<Id<Edge>> = faces.iter().flat_map(|&f| model.view(f).unwrap().edges()).collect();
    edges.sort_unstable();
    edges.dedup();
    edges
}

#[test]
fn a_second_primitive_leaves_the_firsts_blocks_as_they_were_and_unmeshed() {
    let (cube, _) = built(json!([{"op": "box", "size": [10, 10, 10]}]));
    let before = whole(&cube);
    let bolt = json!([{"op": "cylinder", "radius": 4, "height": 10, "frame": {"origin": [20, 0, 0]}}]);
    let (both, changes) = edited(&cube, bolt);
    let again = remesh(&Exec::serial(), &both, &before, &changes);
    let cylinder = faces_new(&both, &cube);
    assert_eq!(cylinder.len(), 4, "two halves of its side and two caps");
    assert_eq!(again.meshed, cylinder, "only the cylinder's faces are meshed");
    assert_eq!(again.sampled, their_edges(&both, &cylinder), "and only its edges sampled");
    let (was, now) = (Buffers::of(&before), Buffers::of(&again.mesh));
    for face in cube.ids::<Face>() {
        assert_eq!(block(&was, face), block(&now, face), "{face:?} keeps its block bit for bit");
    }
    assert_eq!(again.mesh.digest(), whole(&both).digest(), "the mesh the model gives meshed anew");
}

#[test]
fn a_body_moved_is_meshed_again_and_the_other_keeps_its_blocks() {
    let steps = json!([
        {"op": "box", "size": [10, 10, 10]},
        {"op": "cylinder", "radius": 4, "height": 10, "frame": {"origin": [20, 0, 0]}}
    ]);
    let (both, _) = built(steps);
    let before = whole(&both);
    let (moved, changes) =
        edited(&both, json!([{"op": "transform", "target": "body:1/1", "frame": {"origin": [0, 30, 0]}}]));
    let again = remesh(&Exec::serial(), &moved, &before, &changes);
    let cylinder: Vec<Id<Face>> = moved.ids::<Face>().into_iter().skip(6).collect();
    assert_eq!(again.meshed, cylinder, "the cylinder keeps its ids and is meshed where it went");
    let (was, now) = (Buffers::of(&before), Buffers::of(&again.mesh));
    for face in moved.ids::<Face>().into_iter().take(6) {
        assert_eq!(block(&was, face), block(&now, face), "{face:?} keeps its block bit for bit");
    }
    assert_ne!(block(&was, cylinder[0]), block(&now, cylinder[0]));
    assert_eq!(again.mesh.digest(), whole(&moved).digest());
}

#[test]
fn a_face_refused_keeps_its_refusal_unmeshed_when_a_primitive_is_added() {
    let (model, _) = built(sector_with_a_hole());
    let before = whole(&model);
    assert_eq!(before.refused.len(), 2, "both caps");
    let (more, changes) = edited(&model, json!([{"op": "box", "size": [1, 1, 1], "frame": {"origin": [-5, 0, 0]}}]));
    let again = remesh(&Exec::serial(), &more, &before, &changes);
    assert_eq!(again.meshed, faces_new(&more, &model), "the box's faces alone");
    assert_eq!(again.mesh.refused, before.refused, "the caps' refusals kept");
    assert_eq!(again.mesh.digest(), whole(&more).digest());
}
