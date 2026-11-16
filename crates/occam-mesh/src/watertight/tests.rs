//! Source: original

use occam_report::{Code, EntityRef, Fault, NamedValue, Reason, Value};
use occam_topo::{Body, Edge, Face, Id};
use occam_units::{Angle, Length, Point, Vec3, World};

use super::*;
use crate::limits::Limits;
use crate::merge::mesh;
use crate::testing::{Layout, collapsed_patch, disc, disc_drawn, model, plane_and_cylinder, sphere_half, undrawn};
use occam_math::Exec;

fn limits(deflection: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap()
}

/// The model of `layouts` meshed at `deflection`, and each layout's edges.
fn meshed(layouts: &[Layout], deflection: f64) -> (Snapshot, Mesh, Vec<Vec<Id<Edge>>>) {
    let (snapshot, edges) = model(layouts);
    let meshed = mesh(&Exec::serial(), &snapshot, limits(deflection));
    (snapshot, meshed, edges)
}

/// The number `name` of `report`.
fn number(report: &Report, name: &str) -> f64 {
    report.value(name).and_then(NamedValue::number).unwrap_or(f64::NAN)
}

/// The point `name` of `report`.
fn point(report: &Report, name: &str) -> Option<[f64; 3]> {
    match report.value(name)?.value {
        Value::Point(at) => Some(at),
        Value::Scalar(_) => None,
    }
}

/// What each report says: its fault, its reason and the entities it names.
fn said(reports: &[Report]) -> Vec<(Option<Fault>, Option<Reason>, Vec<EntityRef>)> {
    assert!(reports.iter().all(|report| report.code == Code::NotWatertight), "{reports:#?}");
    reports.iter().map(|report| (report.fault, report.reason, report.entities.clone())).collect()
}

/// `p` moved one ulp up along `x`.
fn nudged(p: Point<World>) -> Point<World> {
    let v = p.to_vec3();
    Point::from_vec3(Vec3::new(v.x.next_up(), v.y, v.z))
}

/// The vertex in the middle of the use of `edge` by the face of `block`.
fn middle(mesh: &Mesh, block: usize, edge: Id<Edge>) -> usize {
    let side = mesh.blocks[block].sides.iter().find(|side| side.edge == edge).unwrap();
    usize::try_from(side.vertices[side.vertices.len() / 2]).unwrap()
}

#[test]
fn the_tests_sheets_mesh_watertight_open_only_along_the_edges_one_face_uses() {
    for deflection in [0.1, 0.01] {
        for layout in [plane_and_cylinder(2.0), collapsed_patch(), sphere_half(2.0), disc_drawn(1.0)] {
            let (snapshot, mesh, _) = meshed(&[layout], deflection);
            assert_eq!(mesh.blocks.len(), snapshot.ids::<Face>().len(), "every face meshed");
            assert_eq!((mesh.refused.clone(), mesh.open.clone()), (vec![], vec![]), "at {deflection}");
        }
    }
}

#[test]
fn a_boundary_vertex_moved_one_ulp_is_caught_and_named_by_its_edge() {
    let (snapshot, mut mesh, edges) = meshed(&[plane_and_cylinder(2.0)], 0.01);
    let shared = edges[0][3];
    let faces = snapshot.view(shared).unwrap().faces();
    let k = middle(&mesh, 0, shared);
    let before = mesh.points[k];
    mesh.points[k] = nudged(before);
    let found = watertight(&snapshot, &mesh);
    let named = vec![shared.entity(), faces[0].entity(), faces[1].entity()];
    assert_eq!(said(&found), [(Some(Fault::Internal), Some(Reason::SegmentsUnmatched), named)]);
    assert_eq!(number(&found[0], "unmatched"), 4.0, "the segments either side of it, in each face");
    let ends = [point(&found[0], "from").unwrap(), point(&found[0], "to").unwrap()];
    let at = |p: Point<World>| [p.to_vec3().x, p.to_vec3().y, p.to_vec3().z];
    assert!(ends.contains(&at(before)) || ends.contains(&at(mesh.points[k])), "{ends:?}");
    mesh.points[k] = before;
    assert_eq!(watertight(&snapshot, &mesh), [], "moved back, it is watertight again");
}

#[test]
fn a_vertex_of_a_sheets_rim_moved_one_ulp_leaves_it_open_only_where_it_was_open() {
    let (snapshot, mut mesh, edges) = meshed(&[plane_and_cylinder(2.0)], 0.01);
    let k = middle(&mesh, 0, edges[0][1]);
    mesh.points[k] = nudged(mesh.points[k]);
    assert_eq!(watertight(&snapshot, &mesh), [], "one face uses that edge");
}

#[test]
fn an_unmeshed_face_shows_as_a_named_hole() {
    let (snapshot, mesh, edges) = meshed(&[undrawn(plane_and_cylinder(2.0), 0), disc(1.0)], 0.01);
    let faces = snapshot.ids::<Face>();
    assert_eq!(mesh.refused.len(), 2, "the plane, whose edges are not drawn in it, and the disc");
    assert_eq!(mesh.blocks.iter().map(|block| block.face).collect::<Vec<_>>(), [faces[1]]);
    let holes = vec![
        (Some(Fault::NotImplemented), Some(Reason::FaceMissing), vec![faces[0].entity(), edges[0][3].entity()]),
        (Some(Fault::NotImplemented), Some(Reason::FaceMissing), vec![faces[2].entity()]),
    ];
    assert_eq!(said(&mesh.open), holes);
    let shared = mesh.edges.get(edges[0][3]).unwrap().unwrap();
    assert_eq!(
        number(&mesh.open[0], "unmatched"),
        f64::from(u32::try_from(shared.points.len() - 1).unwrap()),
        "its other side"
    );
    assert_eq!(number(&mesh.open[1], "unmatched"), 0.0, "nothing meets the disc");
}

#[test]
fn a_triangle_turned_over_inside_a_face_is_named_by_the_face() {
    let (snapshot, mut mesh, _) = meshed(&[plane_and_cylinder(2.0)], 0.01);
    let bent = &mesh.blocks[1];
    let rim: Vec<u32> = bent.sides.iter().flat_map(|side| side.vertices.iter().copied()).collect();
    let range = usize::try_from(bent.triangles.start).unwrap()..usize::try_from(bent.triangles.end).unwrap();
    let inside = range.into_iter().find(|&t| mesh.triangles[t].iter().all(|k| !rim.contains(k))).unwrap();
    let face = bent.face;
    let [a, b, c] = mesh.triangles[inside];
    mesh.triangles[inside] = [a, c, b];
    let found = watertight(&snapshot, &mesh);
    assert_eq!(said(&found), [(Some(Fault::Internal), Some(Reason::SegmentsUnmatched), vec![face.entity()])]);
    assert_eq!(number(&found[0], "unmatched"), 6.0, "its three segments, and its neighbours' three");
}

#[test]
fn a_shell_with_a_piece_too_many_has_the_wrong_characteristic() {
    let (snapshot, mut mesh, _) = meshed(&[plane_and_cylinder(2.0)], 0.1);
    let first = u32::try_from(mesh.points.len()).unwrap();
    for (x, y, z) in [(5.0, 0.0, 0.0), (6.0, 0.0, 0.0), (5.0, 1.0, 0.0), (5.0, 0.0, 1.0)] {
        mesh.points.push(Point::from_vec3(Vec3::new(x, y, z)));
    }
    let last = mesh.blocks.last_mut().unwrap();
    last.vertices.end += 4;
    let tetrahedron = [[0, 2, 1], [0, 1, 3], [1, 2, 3], [0, 3, 2]];
    mesh.triangles.extend(tetrahedron.map(|t: [u32; 3]| t.map(|k| first + k)));
    last.triangles.end += 4;
    let found = watertight(&snapshot, &mesh);
    let body = snapshot.ids::<Body>()[0];
    let shell = snapshot.view(body).unwrap().shells()[0];
    let named = vec![shell.entity()];
    assert_eq!(said(&found), [(Some(Fault::Internal), Some(Reason::CharacteristicDiffers), named)]);
    assert_eq!((number(&found[0], "found"), number(&found[0], "expected")), (3.0, 1.0), "a disc and a sphere");
}

#[test]
fn a_block_whose_corner_is_not_in_the_mesh_leaves_its_neighbours_unmatched_and_does_not_panic() {
    let (snapshot, mut mesh, _) = meshed(&[plane_and_cylinder(2.0)], 0.1);
    let t = usize::try_from(mesh.blocks[1].triangles.start).unwrap();
    mesh.triangles[t][0] = u32::MAX;
    let found = watertight(&snapshot, &mesh);
    assert_ne!(found, Vec::<Report>::new());
    assert!(found.iter().all(|report| report.reason == Some(Reason::SegmentsUnmatched)), "{found:#?}");
}
