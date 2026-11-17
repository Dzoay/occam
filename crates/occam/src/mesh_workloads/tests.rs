//! Source: original
//! P9's parts drawn and built: their counts and every number drawn, the models of each kind recorded by digest, each
//! meshed whole, and what `gates perf` meshes and picks.

use core::f64::consts::FRAC_PI_4;

use occam_math::Digest;
use occam_mesh::{Limits, mesh};

use super::parts::Frame;
use super::*;

/// The digest of `text`, as sixteen hexadecimal digits.
fn hashed(text: &str) -> String {
    let mut digest = Digest::EMPTY;
    digest.write_bytes(text.as_bytes());
    format!("{:016x}", digest.finish())
}

/// The mesh of `model` at P9's angle and `deflection`, which must be whole: every face meshed and nothing open.
fn whole(model: &Snapshot, deflection: f64) -> occam_mesh::Mesh {
    let limits = Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap();
    let meshed = mesh(&Exec::serial(), model, limits);
    assert!(meshed.refused.is_empty(), "{:?}", meshed.refused);
    assert!(meshed.open.is_empty(), "{:?}", meshed.open);
    meshed
}

#[test]
fn p9s_targets_draw_p9s_parts() {
    let counted = |target| parts(target, P9_SEED).iter().map(|part| part.shape.p9_faces()).sum::<usize>();
    assert_eq!(P9_TARGETS.map(counted), [1002, 10_001]);
}

#[test]
fn p9s_parts_are_drawn_to_the_last_number_as_recorded() {
    // Every size, place and turn of the 1,002-face model's parts, as `Debug` writes each number, exactly.
    assert_eq!(hashed(&format!("{:?}", parts(1000, P9_SEED))), "8baf170fcb2cee1d");
}

#[test]
fn drawing_stops_once_p9_would_have_counted_the_target_and_not_before() {
    let first: Vec<usize> = parts(30, P9_SEED).iter().map(|part| part.shape.p9_faces()).collect();
    let two = first[0] + first[1];
    assert_eq!(parts(two, P9_SEED).len(), 2);
    assert_eq!(parts(two + 1, P9_SEED).len(), 3);
}

#[test]
fn each_kind_of_part_a_request_makes_is_made_as_recorded() {
    // The first plate and the first of each solid of revolution P9 drew: their steps, and the model they make.
    let drawn = parts(1000, P9_SEED);
    let kinds: [fn(&Shape) -> bool; 6] = [
        |s| matches!(s, Shape::Plate { .. }),
        |s| matches!(s, Shape::Sphere { .. }),
        |s| matches!(s, Shape::Torus { .. }),
        |s| matches!(s, Shape::Cylinder { .. }),
        |s| matches!(s, Shape::Cone { .. }),
        |s| matches!(s, Shape::Bolt { .. }),
    ];
    let picked: Vec<&Part> = kinds.iter().map(|kind| drawn.iter().find(|part| kind(&part.shape)).unwrap()).collect();
    let steps: Vec<serde_json::Value> = picked
        .iter()
        .enumerate()
        .flat_map(|(k, part)| request::steps(&part.frame, &format!("part{k}"), &part.shape))
        .collect();
    let panel = Shape::Panel { size: (1.0, 1.0, 1.0), phase: 0.0 };
    assert!(request::steps(&picked[0].frame, "panel", &panel).is_empty(), "no request makes a panel");
    let ran = crate::run(None, &request::request(&steps));
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    let model = ran.model.unwrap();
    assert_eq!(format!("{:016x}", model.digest()), "dd9633c24dcc8058");
    whole(&model, 0.2);
}

#[test]
fn a_pick_ray_starts_a_millimetre_off_its_triangle_s_middle_and_points_into_it() {
    // A box turned so that every face's normal has all three components, which a ray's direction must each turn round.
    let ran = crate::run(
        None,
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "box", "size": [2, 3, 5], "frame": {"origin": [10, 20, 30], "z": [1, 2, 2], "x": [2, -2, 1]}}]}"#,
    );
    let meshed = perf_mesh(&Exec::serial(), &ran.model.unwrap()).unwrap();
    let rays = pick_rays(&meshed);
    assert_eq!(rays.len(), meshed.triangles.len().div_ceil(16));
    let picker = Picker::new(&meshed);
    for (ray, triangle) in rays.iter().zip(meshed.triangles.iter().step_by(16)) {
        let [a, b, c] = triangle.map(|k| meshed.points[usize::try_from(k).unwrap()].to_vec3());
        let normal = (b - a).cross(c - a);
        let middle = (a + b + c) * (1.0 / 3.0);
        assert!((ray.direction.norm() - 1.0).abs() < 1e-12);
        assert!((ray.at(1.0) - middle).norm() < 1e-12, "{ray:?}");
        assert!(ray.direction.dot(b - a).abs() < 1e-12 && ray.direction.dot(c - a).abs() < 1e-12);
        assert!(ray.direction.dot(normal) < 0.0, "into the face, against its outward normal");
        let hit = picker.pick(ray, Length::from_millimetres(0.01)).unwrap();
        assert!((hit.distance.millimetres() - 1.0).abs() < 1e-9, "{hit:?}");
    }
}

#[test]
fn the_first_of_p9s_parts_are_a_plate_a_panel_and_another_plate_as_recorded() {
    let drawn = parts(20, P9_SEED);
    let kinds: Vec<bool> = drawn.iter().map(|part| matches!(part.shape, Shape::Panel { .. })).collect();
    assert_eq!(kinds, [false, true, false]);
    let model = synthetic(&Exec::serial(), 20, P9_SEED).unwrap();
    // Each plate's faces, and the panel's eight: a top, a bottom, four walls and its hole's wall in two halves.
    assert_eq!(face_count(&model), 28);
    assert_eq!(format!("{:016x}", model.digest()), "3b83f6d69d93df11");
    whole(&model, 0.2);
    // What `gates perf` meshes in the browser is this mesh, by its digest.
    let perf = PerfParts::of(model.clone()).unwrap();
    assert_eq!(perf.mesh(), whole(&model, 0.1).digest());
    assert_eq!(format!("{:016x}", perf.pick()), "b0fa93d4715113d0");
}

#[test]
fn a_rational_quilt_is_a_closed_solid_of_its_patches() {
    // A quilt of rational quadratic patches, the smallest across that closes; P9's cubic quilts take the same path but
    // for their weights. The commit's checks of spline faces make a larger one slow in a debug build.
    let frame = Frame { origin: [10.0, 20.0, 0.0], turn: FRAC_PI_4 };
    let quilt = Part { frame, shape: Shape::Quilt { across: (3, 3), radius: 2.0, degree: 2, seed: 11 } };
    let model = written(&Exec::serial(), &Snapshot::empty(tolerance().unwrap()), &[&quilt]).unwrap();
    assert_eq!(face_count(&model), 9);
    assert_eq!(format!("{:016x}", model.digest()), "2feaa467409deb9d");
    whole(&model, 0.2);
}
