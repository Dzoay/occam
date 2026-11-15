//! Source: original

use core::f64::consts::PI;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use occam_report::{Code, DisplayUnits, NamedValue, RemedyKind, followed_remedy};
use occam_units::{Angle, Length, Vec3};

use super::*;
use crate::deviation::deviation;
use crate::edges::sample_edges;
use crate::testing::{Layout, collapsed_patch, disc_drawn, model, plane_and_cylinder, sphere_half, undrawn};

fn limits(deflection: f64, angle: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(angle)).unwrap()
}

/// The model of `layouts`, its edges sampled to `limits` and its faces meshed against them, and its faces in id order.
fn meshed(layouts: &[Layout], limits: Limits) -> (Snapshot, Edges, Faces, Vec<Id<Face>>) {
    let (snapshot, _) = model(layouts);
    let edges = sample_edges(&Exec::serial(), &snapshot, limits);
    let faces = mesh_faces(&Exec::serial(), &snapshot, &edges);
    let ids = snapshot.ids::<Face>();
    (snapshot, edges, faces, ids)
}

/// The mesh of `face`, which must have meshed.
fn of(faces: &Faces, face: Id<Face>) -> &FaceMesh {
    faces.get(face).unwrap().unwrap()
}

/// A point's bits.
fn bits(p: Point<World>) -> [u64; 3] {
    let v = p.to_vec3();
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// The positions of vertices `at` of `mesh`.
fn placed(mesh: &FaceMesh, at: &[u32]) -> Vec<[u64; 3]> {
    at.iter().map(|&k| bits(mesh.points[usize::try_from(k).unwrap()])).collect()
}

/// The normal of the triangle with corners at `p`, `q`, `r`, unnormalised.
fn across(p: Point<World>, q: Point<World>, r: Point<World>) -> Vec3 {
    (q - p).to_vec3().cross((r - p).to_vec3())
}

/// What every face's mesh holds: a position, a unit normal and a place in the face's parameters at each vertex;
/// triangles of three distinct positions, facing the way their corners' normals do; and each use's vertices its
/// edge's samples, bit for bit, in the use's direction.
fn holds(edges: &Edges, mesh: &FaceMesh) {
    let n = mesh.points.len();
    assert_eq!((mesh.normals.len(), mesh.uvs.len()), (n, n));
    assert_ne!(mesh.triangles.len(), 0);
    for normal in &mesh.normals {
        assert!((normal.get().norm() - 1.0).abs() < 1e-12, "{normal:?}");
    }
    for triangle in &mesh.triangles {
        let corners = triangle.map(|k| usize::try_from(k).unwrap());
        let [p, q, r] = corners.map(|k| mesh.points[k]);
        assert!(p != q && q != r && r != p, "{triangle:?} has two corners at one place");
        let shading = corners.iter().fold(Vec3::new(0.0, 0.0, 0.0), |sum, &k| sum + mesh.normals[k].get());
        assert!(across(p, q, r).dot(shading) > 0.0, "{triangle:?} faces against its normals");
    }
    for side in &mesh.sides {
        let samples = edges.get(side.edge).unwrap().unwrap();
        let mut along: Vec<[u64; 3]> = samples.points.iter().map(|&p| bits(p)).collect();
        if side.reversed {
            along.reverse();
        }
        assert_eq!(placed(mesh, &side.vertices), along, "the use of {:?}", side.edge);
    }
}

#[test]
fn a_plane_and_a_cylinder_mesh_within_their_deflection_measured_to_the_triangle() {
    for (deflection, angle) in [(0.1, PI), (0.01, PI), (1e-3, PI), (0.01, 0.35)] {
        let (snapshot, edges, faces, ids) = meshed(&[plane_and_cylinder(2.0)], limits(deflection, angle));
        assert_eq!(faces.refused().count(), 0);
        for &face in &ids {
            let mesh = of(&faces, face);
            holds(&edges, mesh);
            let surface = snapshot.view(face).unwrap().surface().unwrap();
            let off = deviation(mesh, surface);
            assert!(off <= deflection, "{face:?} at {deflection}, {angle}: {off}");
            assert!(!mesh.capped);
        }
        let flat = of(&faces, ids[0]);
        let boundary: usize = flat.sides.iter().map(|s| s.vertices.len() - 1).sum();
        assert_eq!(flat.points.len(), boundary, "a plane is not refined");
        assert!(flat.normals.iter().all(|n| n.get() == Vec3::new(0.0, 0.0, 1.0)), "the plane faces +Z");
    }
}

#[test]
fn neighbours_use_each_mesh_edge_of_the_edge_they_share_once_each_and_opposite_ways() {
    let (snapshot, edges, faces, ids) = meshed(&[plane_and_cylinder(2.0)], limits(0.01, 0.35));
    let mut uses: BTreeMap<([u64; 3], [u64; 3]), i32> = BTreeMap::new();
    for &face in &ids {
        let mesh = of(&faces, face);
        for triangle in &mesh.triangles {
            let [p, q, r] = triangle.map(|k| bits(mesh.points[usize::try_from(k).unwrap()]));
            for (a, b) in [(p, q), (q, r), (r, p)] {
                *uses.entry((a, b)).or_default() += 1;
            }
        }
    }
    let shared =
        snapshot.view(ids[0]).unwrap().edges().into_iter().find(|e| snapshot.view(*e).unwrap().faces().len() == 2);
    let samples = edges.get(shared.unwrap()).unwrap().unwrap();
    for pair in samples.points.windows(2) {
        let (a, b) = (bits(pair[0]), bits(pair[1]));
        assert_eq!((uses.get(&(a, b)), uses.get(&(b, a))), (Some(&1), Some(&1)), "{pair:?}");
    }
}

#[test]
fn p9s_bicubic_patch_with_a_collapsed_row_meshes_within_its_deflection() {
    for deflection in [1e-2, 3e-3] {
        let (snapshot, edges, faces, ids) = meshed(&[collapsed_patch()], limits(deflection, 0.35));
        let mesh = of(&faces, ids[0]);
        holds(&edges, mesh);
        let off = deviation(mesh, snapshot.view(ids[0]).unwrap().surface().unwrap());
        assert!(off <= deflection, "at {deflection}: {off}");
        // The row at `v = 0` is one point, where the loop crosses from `u = 0` to `u = 1`: two vertices there.
        let collapsed = mesh.points.iter().filter(|p| p.to_vec3() == Vec3::new(0.5, 0.0, 0.0)).count();
        assert_eq!(collapsed, 2);
    }
}

#[test]
fn a_disc_covers_its_area_less_what_its_chords_cut_off() {
    let (_, edges, faces, ids) = meshed(&[disc_drawn(1.0)], limits(1e-3, PI));
    let mesh = of(&faces, ids[0]);
    holds(&edges, mesh);
    let twice = |t: &[u32; 3]| {
        let [p, q, r] = t.map(|k| mesh.points[usize::try_from(k).unwrap()]);
        across(p, q, r).norm()
    };
    let area = mesh.triangles.iter().map(twice).sum::<f64>() / 2.0;
    // Each chord is within half the deflection of the circle, so the polygon holds the disc of radius 1 - 5e-4.
    assert!(PI * (1.0 - 5e-4) * (1.0 - 5e-4) < area && area < PI, "{area}");
    assert_eq!(mesh.sides.len(), 1);
    assert_eq!(mesh.sides[0].vertices.first(), mesh.sides[0].vertices.last(), "the circle's ends are one vertex");
}

#[test]
fn a_face_whose_edges_are_not_drawn_in_its_parameters_is_named_and_its_neighbour_still_meshes() {
    let (snapshot, edges, faces, ids) = meshed(&[undrawn(plane_and_cylinder(2.0), 0)], limits(0.01, 0.35));
    let report = faces.get(ids[0]).unwrap().unwrap_err();
    assert_eq!(
        (report.code, report.fault, report.reason),
        (Code::FaceNotMeshed, Some(Fault::NotImplemented), Some(Reason::NoParamCurve))
    );
    let first = snapshot.view(ids[0]).unwrap().edges()[0];
    assert_eq!(report.entities, [ids[0].entity(), first.entity()]);
    assert_eq!(
        report.message(DisplayUnits::default()),
        "The face could not be meshed, for the reason given; the faces around it are meshed without it."
    );
    assert_eq!(faces.refused().collect::<Vec<_>>(), [report]);
    let bent = of(&faces, ids[1]);
    holds(&edges, bent);
    assert!(deviation(bent, snapshot.view(ids[1]).unwrap().surface().unwrap()) <= 0.01);
}

#[test]
fn a_face_whose_edge_was_refused_carries_the_edges_remedy_and_meshes_at_it() {
    // A circle of radius 1000 at a deflection of 1e-6 needs six times as many samples as an edge may have.
    let (snapshot, _, faces, ids) = meshed(&[disc_drawn(1000.0)], limits(1e-6, 0.35));
    let report = faces.get(ids[0]).unwrap().unwrap_err().clone();
    let circle = snapshot.view(ids[0]).unwrap().edges()[0];
    assert_eq!((report.code, report.reason), (Code::FaceNotMeshed, Some(Reason::EdgeNotSampled)));
    assert_eq!(report.entities, [ids[0].entity(), circle.entity()]);
    followed_remedy!(report, Code::FaceNotMeshed, RemedyKind::UseDeflection, |remedy| {
        let deflection = remedy.value("deflection").and_then(NamedValue::number).unwrap_or(f64::NAN);
        let edges = sample_edges(&Exec::serial(), &snapshot, limits(deflection, 0.35));
        mesh_faces(&Exec::serial(), &snapshot, &edges).refused().next().map_or(Ok(()), |again| Err(again.clone()))
    });
}

#[test]
fn faces_are_listed_in_id_order_each_once() {
    let (_, _, faces, ids) = meshed(&[disc_drawn(1.0), undrawn(plane_and_cylinder(2.0), 1)], limits(0.01, 0.35));
    assert_eq!(faces.iter().map(|(f, _)| f).collect::<Vec<_>>(), ids);
    assert_eq!(faces.refused().count(), 1, "the cylinder's");
    assert!(faces.get(ids[2]).unwrap().is_err());
}

#[test]
fn refinement_stops_at_its_cap_and_says_so() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0)]);
    let edges = sample_edges(&Exec::serial(), &snapshot, limits(1e-3, 0.35));
    let cylinder = snapshot.ids::<Face>()[1];
    let whole = mesh_face(&snapshot, cylinder, &edges).unwrap();
    let boundary: usize = whole.sides.iter().map(|s| s.vertices.len() - 1).sum();
    let cap = boundary + (whole.triangles.len() - boundary) / 2;
    let capped = mesh_face_within(&snapshot, cylinder, &edges, cap).unwrap();
    assert!(capped.capped && !whole.capped);
    assert!(capped.triangles.len() <= cap && cap < whole.triangles.len(), "{} of {cap}", capped.triangles.len());
}

#[test]
fn the_meshes_are_the_same_bits_at_one_thread_and_at_four() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), collapsed_patch(), disc_drawn(1.0)]);
    let four = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let edges = sample_edges(&Exec::serial(), &snapshot, limits(1e-2, 0.35));
    let words = |faces: &Faces| {
        let mesh = |m: &FaceMesh| {
            let points = m.points.iter().map(|&p| bits(p));
            let normals = m.normals.iter().map(|n| bits(Point::from_vec3(n.get())));
            let uvs = m.uvs.iter().map(|uv| [uv.u().to_bits(), uv.v().to_bits(), 0]);
            let corners = m.triangles.iter().map(|t| t.map(u64::from));
            points.chain(normals).chain(uvs).chain(corners).collect::<Vec<[u64; 3]>>()
        };
        faces.iter().map(|(f, r)| (f, r.map(mesh).map_err(Report::clone))).collect::<Vec<_>>()
    };
    let serial = mesh_faces(&Exec::serial(), &snapshot, &edges);
    assert_eq!(serial.refused().count(), 0);
    assert_eq!(words(&serial), words(&mesh_faces(&four, &snapshot, &edges)));
}

#[test]
fn half_a_sphere_meshes_within_its_deflection_its_normals_its_radii_at_its_poles_too() {
    for deflection in [0.1, 0.01] {
        let (snapshot, edges, faces, ids) = meshed(&[sphere_half(2.0)], limits(deflection, 0.35));
        let mesh = of(&faces, ids[0]);
        holds(&edges, mesh);
        assert!(deviation(mesh, snapshot.view(ids[0]).unwrap().surface().unwrap()) <= deflection);
        let poles = mesh.points.iter().filter(|p| (p.to_vec3().z.abs() - 2.0).abs() < 1e-12).count();
        assert_eq!(poles, 4, "each pole twice, where the loop crosses from one side to the other");
        for (p, n) in mesh.points.iter().zip(&mesh.normals) {
            assert!((n.get() - p.to_vec3() * 0.5).norm() < 1e-12, "at {p:?}: {n:?}");
        }
    }
}
