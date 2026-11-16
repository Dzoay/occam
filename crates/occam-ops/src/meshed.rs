//! Source: original
//! Every face `construction` makes, meshed (view.faces): planes with holes, and the sides and caps of each primitive,
//! extrusion and revolve, each within its deflection measured to the triangle; and a face seeded to fail, named while
//! its neighbours mesh. Every model meshes watertight, and a crack, a hole or a piece too many seeded in a solid's mesh
//! is named (view.watertight). `occam-mesh` sits above this crate in the layers, so its tests on built models live
//! here.

use occam_math::Exec;
use occam_mesh::{Edges, FaceMesh, Faces, Limits, Mesh, deviation, merge, mesh, mesh_faces, sample_edges, watertight};
use occam_report::{Code, Fault, Reason, Report};
use occam_topo::{Body, Edge, Face, Id, Snapshot};
use occam_units::{Angle, Length, Point, Vec3, World};
use serde_json::{Value, json};

use crate::construct::tests::{built, plate};

fn limits(deflection: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap()
}

/// The model `steps` build, its edges sampled to `limits` and its faces meshed against them.
fn meshed(steps: Value, limits: Limits) -> (Snapshot, Edges, Faces) {
    let (model, _) = built(steps);
    let edges = sample_edges(&Exec::serial(), &model, limits);
    let faces = mesh_faces(&Exec::serial(), &model, &edges);
    (model, edges, faces)
}

/// The steps of `first` and then `then`.
fn and(first: &Value, then: &[Value]) -> Value {
    let mut steps = first.as_array().cloned().unwrap_or_default();
    steps.extend(then.iter().cloned());
    Value::Array(steps)
}

/// A face labelled `face` bounded by the circle about `centre` of `radius` facing `normal`.
fn disc(centre: [f64; 3], radius: f64, normal: [f64; 3]) -> Value {
    json!([
        {"op": "wire", "label": "rim", "edges": [{"circle": {"centre": centre, "radius": radius, "normal": normal}}]},
        {"op": "face", "label": "face", "outer": "rim"}
    ])
}

/// A face labelled `face` bounded by the closed polygon through `points`.
fn polygon(points: &Value) -> Value {
    json!([
        {"op": "wire", "label": "rim", "points": points, "closed": true},
        {"op": "face", "label": "face", "outer": "rim"}
    ])
}

/// What revolving the face labelled `face` by `angle` degrees about `axis` through the origin adds.
fn revolved(axis: [f64; 3], angle: f64) -> Value {
    json!({"op": "revolve", "profile": "face", "origin": [0, 0, 0], "axis": axis, "angle": angle})
}

/// A model of every kind of face `construction` makes, each by name.
fn models() -> Vec<(&'static str, Value)> {
    let root = 21.0_f64.sqrt();
    let lens = json!([
        {"op": "wire", "label": "rim", "edges": [{"arc": [[0, 0, -root], [3, 0, 0], [0, 0, root]]}, {"line": [[0, 0, root], [0, 0, -root]]}]},
        {"op": "face", "label": "face", "outer": "rim"}
    ]);
    let section = polygon(&json!([[0, 0, 0], [60, 0, 0], [60, 0, 10], [0, 0, 10]]));
    let bolt = json!({"op": "cylinder", "radius": 4, "height": 10, "frame": {"origin": [30, 20, 0]}});
    vec![
        ("plate", plate()),
        ("plate extruded", and(&plate(), &[json!({"op": "extrude", "face": "plate", "vector": [0, 0, 5]})])),
        ("box", json!([{"op": "box", "size": [10, 20, 30]}])),
        ("wedge", json!([{"op": "wedge", "size": [10, 5, 4], "top": {"start": 2, "width": 3}}])),
        ("wedge to an edge", json!([{"op": "wedge", "size": [10, 5, 4], "top": {"start": 12, "width": 0}}])),
        ("cylinder", json!([{"op": "cylinder", "radius": 4, "height": 10}])),
        ("cone", json!([{"op": "cone", "radius1": 5, "radius2": 2, "height": 10}])),
        ("cone to a point", json!([{"op": "cone", "radius1": 0, "radius2": 3, "height": 4}])),
        (
            "sphere",
            json!([{"op": "sphere", "radius": 2, "frame": {"origin": [1, 2, 3], "z": [1, 1, 1], "x": [1, -1, 0]}}]),
        ),
        ("torus", json!([{"op": "torus", "major": 5, "minor": 1}])),
        (
            "disc extruded aslant",
            and(
                &disc([0.0; 3], 2.0, [0.0, 0.0, 1.0]),
                &[json!({"op": "extrude", "face": "face", "vector": [1, 2, 5]})],
            ),
        ),
        ("lens turned a whole turn", and(&lens, &[revolved([0.0, 0.0, 1.0], 360.0)])),
        (
            "disc turned into a ring",
            and(&disc([10.0, 0.0, 0.0], 3.0, [0.0, 1.0, 0.0]), &[revolved([0.2, 0.0, 1.0], 360.0)]),
        ),
        (
            "triangle turned a quarter",
            and(&polygon(&json!([[0, 0, 2], [4, 0, 2], [0, 0, 12]])), &[revolved([0.0, 0.0, 1.0], 90.0)]),
        ),
        ("the agent's part", and(&section, &[json!({"op": "extrude", "face": "face", "vector": [0, 40, 0]}), bolt])),
    ]
}

/// The position of vertex `k` of `mesh`.
fn at(mesh: &FaceMesh, k: u32) -> Point<World> {
    mesh.points[usize::try_from(k).unwrap()]
}

/// Each triangle of `mesh` with its unnormalised normal and the sum of its corners' normals.
fn facings(mesh: &FaceMesh) -> impl Iterator<Item = ([Point<World>; 3], Vec3, Vec3)> + '_ {
    mesh.triangles.iter().map(|t| {
        let [p, q, r] = t.map(|k| at(mesh, k));
        let shading =
            t.iter().fold(Vec3::new(0.0, 0.0, 0.0), |sum, &k| sum + mesh.normals[usize::try_from(k).unwrap()].get());
        ([p, q, r], (q - p).to_vec3().cross((r - p).to_vec3()), shading)
    })
}

#[test]
fn every_face_construction_makes_meshes_within_its_deflection_measured_to_the_triangle() {
    for deflection in [0.1, 0.03] {
        for (name, steps) in models() {
            let (model, edges, faces) = meshed(steps, limits(deflection));
            assert_eq!(edges.refused().count(), 0, "{name}");
            assert_eq!(faces.refused().collect::<Vec<&Report>>(), Vec::<&Report>::new(), "{name}");
            for (face, mesh) in faces.iter() {
                let mesh = mesh.unwrap();
                let off = deviation(mesh, model.view(face).unwrap().surface().unwrap());
                assert!(off <= deflection, "{name}: {face:?} strays {off} at {deflection}");
                assert!(!mesh.capped && !mesh.triangles.is_empty(), "{name}: {face:?}");
                for (corners, normal, shading) in facings(mesh) {
                    assert!(normal.dot(shading) > 0.0, "{name}: {corners:?} faces against its normals");
                }
            }
        }
    }
}

#[test]
fn a_convex_solids_triangles_and_normals_face_out_of_it_poles_and_apex_included() {
    for (name, steps) in
        models().into_iter().filter(|(name, _)| ["box", "cylinder", "cone", "cone to a point", "sphere"].contains(name))
    {
        let (_, _, faces) = meshed(steps, limits(0.05));
        let meshes: Vec<&FaceMesh> = faces.iter().map(|(_, mesh)| mesh.unwrap()).collect();
        let all = meshes.iter().flat_map(|m| m.points.iter().map(|p| p.to_vec3()));
        let (sum, count) = all.fold((Vec3::new(0.0, 0.0, 0.0), 0.0), |(s, n), p| (s + p, n + 1.0));
        let centre = sum * (1.0 / count);
        for mesh in meshes {
            for ([p, q, r], normal, _) in facings(mesh) {
                let middle = (p.to_vec3() + q.to_vec3() + r.to_vec3()) * (1.0 / 3.0);
                assert!(normal.dot(middle - centre) > 0.0, "{name}: {p:?} {q:?} {r:?} faces in");
            }
            for (p, n) in mesh.points.iter().zip(&mesh.normals) {
                assert!(n.get().dot(p.to_vec3() - centre) > 0.0, "{name}: the normal at {p:?} points in");
            }
        }
    }
}

#[test]
fn a_spheres_normals_are_its_radii_at_its_poles_too() {
    let (_, _, faces) = meshed(json!([{"op": "sphere", "radius": 2}]), limits(0.01));
    for (_, mesh) in faces.iter() {
        let mesh = mesh.unwrap();
        let poles = mesh.points.iter().filter(|p| p.to_vec3().x == 0.0 && p.to_vec3().y == 0.0).count();
        assert_eq!(poles, 4, "each pole twice, where the loop crosses from one side to the other");
        for (p, n) in mesh.points.iter().zip(&mesh.normals) {
            let radial = p.to_vec3() * 0.5;
            assert!((n.get() - radial).norm() < 1e-12, "at {p:?}: {n:?}");
        }
    }
}

/// A sector of radius 10 a sixteenth of a turn either side of +X, extruded: its arc is sampled at its ends and its
/// middle at 0.1, so the chord from (10, 0) sags about 0.048 inside it. A hole of radius 0.25 comes within 1e-3 of the
/// arc under the middle of that chord, so the chord cuts across the hole's samples, in both caps.
fn sector_with_a_hole() -> Value {
    let (x, y) = (9.807_852_804_032_304, 1.950_903_220_161_282_4);
    json!([
        {"op": "wire", "label": "rim", "edges": [{"arc": [[x, -y, 0], [10, 0, 0], [x, y, 0]]}, {"line": [[x, y, 0], [0, 0, 0]]}, {"line": [[0, 0, 0], [x, -y, 0]]}]},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [9.702_055_900_327_249, 0.955_569_101_072_886_4, 0], "radius": 0.25, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "face", "outer": "rim", "holes": ["hole"]},
        {"op": "extrude", "face": "face", "vector": [0, 0, 5]}
    ])
}

#[test]
fn a_face_seeded_to_fail_is_named_with_its_reason_and_its_neighbours_still_mesh() {
    let (model, _, faces) = meshed(sector_with_a_hole(), limits(0.1));
    let refused: Vec<&Report> = faces.refused().collect();
    assert_eq!(refused.len(), 2, "both caps: {refused:#?}");
    for report in refused {
        assert_eq!((report.code, report.reason), (Code::FaceNotMeshed, Some(Reason::BoundaryCrosses)));
        let face = model.ids::<Face>().into_iter().find(|f| f.entity() == report.entities[0]).unwrap();
        let face = model.view(face).unwrap();
        let edges: Vec<_> = report.entities[1..].to_vec();
        assert!(edges.len() >= 2, "the edges whose samples cross: {report:#?}");
        assert!(edges.iter().all(|e| face.edges().iter().any(|f| f.entity() == *e)), "{report:#?}");
    }
    let meshed: Vec<_> = faces.iter().filter_map(|(face, mesh)| mesh.ok().map(|m| (face, m))).collect();
    assert_eq!(meshed.len(), 5, "the arc's side, the two flat sides and the hole's side, in two halves");
    for (face, mesh) in meshed {
        assert!(deviation(mesh, model.view(face).unwrap().surface().unwrap()) <= 0.1, "{face:?}");
    }
}

/// The model `steps` build, meshed whole to `deflection`.
fn whole(steps: Value, deflection: f64) -> (Snapshot, Mesh) {
    let (model, _) = built(steps);
    let made = mesh(&Exec::serial(), &model, limits(deflection));
    (model, made)
}

/// The reason each report gives and the entities it names, every one `mesh.not-watertight`.
fn holes_and_cracks(reports: &[Report]) -> Vec<(Option<Reason>, Vec<occam_report::EntityRef>)> {
    assert!(reports.iter().all(|report| report.code == Code::NotWatertight), "{reports:#?}");
    reports.iter().map(|report| (report.reason, report.entities.clone())).collect()
}

/// `edges` and the faces each one's uses lie in, as a report on it names them.
fn with_faces(model: &Snapshot, edges: &[Id<Edge>]) -> Vec<(Option<Reason>, Vec<occam_report::EntityRef>)> {
    let named = |&edge: &Id<Edge>| {
        let faces = model.view(edge).unwrap().faces().into_iter().map(Id::entity);
        (Some(Reason::SegmentsUnmatched), std::iter::once(edge.entity()).chain(faces).collect())
    };
    edges.iter().map(named).collect()
}

#[test]
fn every_primitive_and_swept_solid_meshes_watertight() {
    for deflection in [0.1, 0.03] {
        for (name, steps) in models() {
            let (model, made) = whole(steps, deflection);
            assert_eq!(made.blocks.len(), model.ids::<Face>().len(), "{name}: every face has its block");
            assert_eq!(made.refused, [], "{name}");
            assert_eq!(made.open, [], "{name} at {deflection}");
        }
    }
}

#[test]
fn a_box_corner_moved_one_ulp_in_one_face_is_named_by_the_two_edges_it_ends() {
    let (model, mut made) = whole(json!([{"op": "box", "size": [10, 20, 30]}]), 0.1);
    let sides = &made.blocks[2].sides;
    let (Some(first), Some(last)) = (sides.first(), sides.last()) else { panic!("{sides:?}") };
    let mut ended = [first.edge, last.edge];
    ended.sort_unstable();
    let corner = usize::try_from(first.vertices[0]).unwrap();
    let p = made.points[corner].to_vec3();
    made.points[corner] = Point::from_vec3(Vec3::new(p.x, p.y, p.z.next_down()));
    let found = watertight(&model, &made);
    assert_eq!(holes_and_cracks(&found), with_faces(&model, &ended));
    assert!(found.iter().all(|report| report.fault == Some(Fault::Internal)));
}

#[test]
fn a_solid_whose_caps_will_not_mesh_has_two_holes_each_named_with_its_edges() {
    let (model, made) = whole(sector_with_a_hole(), 0.1);
    let caps: Vec<Id<Face>> = made.refused.iter().map(|r| Id::from_entity(r.entities[0]).unwrap()).collect();
    assert_eq!(caps.len(), 2, "both caps are refused: {:#?}", made.refused);
    let rims = caps.iter().map(|&cap| {
        let mut edges = model.view(cap).unwrap().edges();
        edges.sort_unstable();
        let named = std::iter::once(cap.entity()).chain(edges.into_iter().map(Id::entity)).collect();
        (Some(Reason::FaceMissing), named)
    });
    assert_eq!(holes_and_cracks(&made.open), rims.collect::<Vec<_>>());
    assert!(made.open.iter().all(|report| report.fault == Some(Fault::InvalidInput)), "as the caps were refused");
    let again = merge(made.edges.clone(), &mesh_faces(&Exec::serial(), &model, &made.edges));
    assert_eq!(watertight(&model, &again), made.open, "the check is the same on what merge gives");
}

#[test]
fn a_solids_mesh_with_a_piece_too_many_has_the_wrong_characteristic() {
    let (model, mut made) = whole(json!([{"op": "cylinder", "radius": 4, "height": 10}]), 0.1);
    let shell = model.view(model.ids::<Body>()[0]).unwrap().shells()[0];
    let n = u32::try_from(made.points.len()).unwrap();
    let apex = [[20.0, 0.0, 0.0], [21.0, 0.0, 0.0], [20.0, 1.0, 0.0], [20.0, 0.0, 1.0]];
    made.points.extend(apex.map(|[x, y, z]| Point::<World>::from_vec3(Vec3::new(x, y, z))));
    made.triangles.extend([[n, n + 2, n + 1], [n, n + 1, n + 3], [n + 1, n + 2, n + 3], [n, n + 3, n + 2]]);
    let block = made.blocks.last_mut().unwrap();
    (block.vertices.end, block.triangles.end) = (block.vertices.end + 4, block.triangles.end + 4);
    let found = watertight(&model, &made);
    assert_eq!(holes_and_cracks(&found), [(Some(Reason::CharacteristicDiffers), vec![shell.entity()])]);
    let counts = ["found", "expected"].map(|name| found[0].value(name).and_then(occam_report::NamedValue::number));
    assert_eq!(counts, [Some(4.0), Some(2.0)], "a sphere's and a tetrahedron's, against a sphere's");
}
