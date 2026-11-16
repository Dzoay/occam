//! Source: original

use core::num::NonZeroUsize;

use occam_math::Exec;
use occam_report::followed_remedy;
use occam_topo::Snapshot;
use occam_units::Angle;

use super::*;
use crate::limits::Limits;
use crate::merge::mesh;
use crate::testing::{disc_drawn, model, plane_and_cylinder, sphere_half, square};

fn at() -> Limits {
    Limits::new(Length::from_millimetres(0.01), Angle::from_radians(0.35)).unwrap()
}

/// `model` meshed on `exec` and made ready to pick.
fn ready(exec: &Exec, model: &Snapshot) -> Picker {
    Picker::new(&mesh(exec, model, at()))
}

fn ray(origin: [f64; 3], direction: [f64; 3]) -> Ray {
    Ray::new(origin, direction).unwrap()
}

fn hit(picker: &Picker, ray: &Ray, tolerance: f64) -> Result<Hit, Report> {
    picker.pick(ray, Length::from_millimetres(tolerance))
}

/// What `ray` hits, by id, within `tolerance`.
fn picked(picker: &Picker, ray: &Ray, tolerance: f64) -> EntityRef {
    hit(picker, ray, tolerance).unwrap().entity
}

/// The square and the quarter cylinder of radius 2, ready to pick, with the square's face, the cylinder's and their
/// edges.
fn square_and_cylinder() -> (Picker, [EntityRef; 2], Vec<EntityRef>) {
    let (snapshot, edges) = model(&[plane_and_cylinder(2.0)]);
    let [square, cylinder] = snapshot.ids::<Face>()[..] else { panic!("two faces") };
    let edges = edges[0].iter().map(|edge| edge.entity()).collect();
    (ready(&Exec::serial(), &snapshot), [square.entity(), cylinder.entity()], edges)
}

#[test]
fn a_ray_down_through_the_middle_of_a_face_hits_it_where_it_crosses_and_one_through_a_shared_side_too() {
    let (picker, [square, cylinder], _) = square_and_cylinder();
    let down = hit(&picker, &ray([0.25, 0.4, 5.0], [0.0, 0.0, -2.0]), 0.0).unwrap();
    assert_eq!(
        down,
        Hit {
            entity: square,
            point: Point::from_vec3(Vec3::new(0.25, 0.4, 0.0)),
            distance: Length::from_millimetres(5.0)
        }
    );
    assert!(picker.triangles.iter().filter(|t| t.face == 0).count() >= 2, "the square is two triangles or more");
    for k in 1..20 {
        let x = f64::from(k) / 20.0;
        assert_eq!(picked(&picker, &ray([x, x, 3.0], [0.0, 0.0, -1.0]), 0.0), square, "along the diagonal at {x}");
        assert_eq!(picked(&picker, &ray([x, 1.0 - x, -3.0], [0.0, 0.0, 1.0]), 0.0), square, "from below at {x}");
    }
    let side = hit(&picker, &ray([-1.0, 0.5, 5.0], [0.0, 0.0, -1.0]), 0.0).unwrap();
    assert_eq!(side.entity, cylinder);
    let z = -2.0 + 3.0_f64.sqrt();
    assert!((side.point.to_vec3() - Vec3::new(-1.0, 0.5, z)).norm() < 0.01, "on the cylinder, within the deflection");
}

#[test]
fn an_edge_within_the_tolerance_wins_over_its_face_and_one_just_beyond_it_does_not() {
    let (picker, [square, _], edges) = square_and_cylinder();
    let near = hit(&picker, &ray([0.5, 0.009, 4.0], [0.0, 0.0, -1.0]), 0.01).unwrap();
    assert_eq!(near.entity, edges[0], "the square's side along y = 0");
    assert_eq!(near.point, Point::from_vec3(Vec3::new(0.5, 0.0, 0.0)), "its point nearest the ray");
    assert_eq!(picked(&picker, &ray([0.5, 0.0101, 4.0], [0.0, 0.0, -1.0]), 0.01), square, "just beyond the tolerance");
    assert_eq!(picked(&picker, &ray([0.5, 0.009, 4.0], [0.0, 0.0, -1.0]), 0.0), square, "no tolerance, no edge");
    assert_eq!(picked(&picker, &ray([0.5, -0.009, 4.0], [0.0, 0.0, -1.0]), 0.01), edges[0], "beside the square");
    let corner = picked(&picker, &ray([0.997, 0.008, 4.0], [0.0, 0.0, -1.0]), 0.01);
    assert_eq!(corner, edges[1], "of two edges in reach, the one nearer the ray");
}

#[test]
fn an_edge_of_the_face_hit_wins_where_the_ray_hits_within_the_tolerance_of_it_however_far_on_the_ray_nears_it() {
    let (picker, [square, _], edges) = square_and_cylinder();
    // Nearly along the square's side at y = 0, descending onto the square 0.005 from it: the ray comes nearest the
    // side's line well past where it hits the square.
    let way = [-0.99, -0.1, -0.1];
    let landing = Vec3::new(0.5, 0.005, 0.0);
    let unit = ray([0.0; 3], way).direction;
    let start = landing - unit * 2.0;
    let grazing = ray([start.x, start.y, start.z], way);
    let face = picker.first_face(&grazing).unwrap();
    let near = approach(&grazing, picker.segment((0, 0)), (0, 0));
    assert!(near.along > face.0 + 0.01 && near.apart <= 0.01, "{near:?} past {face:?}");
    assert_eq!(picked(&picker, &grazing, 0.01), edges[0]);
    assert_eq!(picked(&picker, &grazing, 0.004), square, "where the hit is beyond the tolerance of it");
}

#[test]
fn an_edge_the_ray_nears_only_behind_the_face_it_hits_loses_to_that_face() {
    let (picker, [_, cylinder], edges) = square_and_cylinder();
    // Down past the cylinder's bottom edge at (-2, y, -2), 0.005 from it, hitting the cylinder 0.14 above it.
    let down = ray([-1.995, 0.5, 5.0], [0.0, 0.0, -1.0]);
    assert_eq!(picked(&picker, &down, 0.01), cylinder);
    assert_eq!(picked(&picker, &down, 0.2), edges[6], "with the hit itself within reach of it");
}

#[test]
fn two_faces_hit_at_one_distance_go_to_the_smaller_id() {
    let (snapshot, _) = model(&[disc_drawn(1.0), disc_drawn(1.0)]);
    let [first, second] = snapshot.ids::<Face>()[..] else { panic!("two discs") };
    assert!(first < second);
    let picker = ready(&Exec::serial(), &snapshot);
    for origin in [[0.2, 0.3, 4.0], [0.2, 0.3, -4.0], [-0.5, 0.1, 1.0]] {
        let way = [0.0, 0.0, -origin[2]];
        assert_eq!(picked(&picker, &ray(origin, way), 0.0), first.entity(), "from {origin:?}");
    }
}

#[test]
fn a_ray_that_hits_nothing_is_refused_and_aiming_at_the_point_given_hits() {
    let (picker, _, _) = square_and_cylinder();
    let start = [3.0, 4.0, 5.0];
    let report = hit(&picker, &ray(start, [0.0, 0.0, 1.0]), 0.1).unwrap_err();
    assert_eq!(report.code, Code::RayMissed);
    assert_eq!(report.value("tolerance").and_then(NamedValue::number), Some(0.1));
    followed_remedy!(report, Code::RayMissed, RemedyKind::AimAt, |remedy| {
        let Some(occam_report::Value::Point(point)) = remedy.value("point").map(|value| value.value) else {
            return Err(report.clone());
        };
        let way = [point[0] - start[0], point[1] - start[1], point[2] - start[2]];
        hit(&picker, &ray(start, way), 0.0)
    });
}

#[test]
fn a_ray_in_the_plane_of_every_triangle_is_given_nothing_to_aim_at() {
    let (snapshot, _) = model(&[disc_drawn(1.0)]);
    let picker = ready(&Exec::serial(), &snapshot);
    let report = hit(&picker, &ray([5.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 0.0).unwrap_err();
    assert_eq!((report.code, report.remedies.len()), (Code::RayMissed, 0));
    let empty = Picker::new(&mesh(&Exec::serial(), &model(&[]).0, at()));
    assert!(hit(&empty, &ray([0.0; 3], [0.0, 0.0, 1.0]), 1.0).unwrap_err().remedies.is_empty(), "nothing to aim at");
}

#[test]
fn a_ray_with_no_direction_or_a_start_that_is_not_finite_is_refused() {
    assert_eq!(Ray::new([0.0; 3], [0.0; 3]).unwrap_err().code, Code::InputNoDirection);
    assert_eq!(Ray::new([0.0; 3], [f64::NAN, 0.0, 1.0]).unwrap_err().code, Code::InputNoDirection);
    assert_eq!(Ray::new([f64::INFINITY, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap_err().code, Code::NotFinite);
    assert_eq!(ray([1.0, 2.0, 3.0], [0.0, 3.0, 4.0]).direction, Vec3::new(0.0, 0.6, 0.8), "made of length one");
}

/// Rays across `model`'s box from three sides, a grid each.
fn rays() -> Vec<Ray> {
    let steps = (0..9).map(|k| -2.2 + 0.4 * f64::from(k));
    let grid: Vec<(f64, f64)> = steps.clone().flat_map(|a| steps.clone().map(move |b| (a, b))).collect();
    let down = grid.iter().map(|&(x, y)| ray([x, y, 6.0], [0.01, 0.02, -1.0]));
    let across = grid.iter().map(|&(y, z)| ray([-6.0, y, z], [1.0, 0.03, 0.01]));
    let up = grid.iter().map(|&(x, z)| ray([x, -6.0, z], [0.02, 1.0, 0.05]));
    down.chain(across).chain(up).collect()
}

/// The nearest face `picker` crosses along `ray`, by trying every triangle.
fn every_triangle(picker: &Picker, ray: &Ray) -> Option<(f64, u32)> {
    let line = Line::of(ray);
    let crossings = picker.triangles.iter().filter_map(|t| Some((line.crossing(ray, &t.corners)?, t.face)));
    crossings.reduce(|best, next| if next < best { next } else { best })
}

#[test]
fn the_tree_finds_the_face_trying_every_triangle_finds_and_the_same_rays_give_the_same_hits_at_any_thread_count() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), sphere_half(0.5)]);
    let one = ready(&Exec::serial(), &snapshot);
    let four = ready(&Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap(), &snapshot);
    assert!(one.triangles.len() > 200, "enough triangles for a tree of many levels: {}", one.triangles.len());
    let (mut faces, mut digest) = ([0; 3], occam_math::Digest::EMPTY);
    for ray in rays() {
        let found = one.first_face(&ray);
        assert_eq!(found, every_triangle(&one, &ray), "{ray:?}");
        if let Some((_, face)) = found {
            faces[place(face)] += 1;
        }
        let picked = hit(&one, &ray, 0.05);
        assert_eq!(picked, hit(&four, &ray, 0.05), "{ray:?}");
        if let Ok(Hit { entity, point, distance }) = picked {
            digest.write_bytes(entity.text().as_bytes());
            let p = point.to_vec3();
            for x in [p.x, p.y, p.z, distance.millimetres()] {
                digest.write_value(x);
            }
        }
    }
    assert_eq!(faces, [3, 19, 8], "how many rays hit the square, the cylinder and the half sphere first");
    assert_eq!(digest.finish(), HITS, "every hit's id, point and distance, bit for bit, on every target");
}

/// The hash of every hit of [`rays`] within 0.05 mm, as x86-64 gives it.
const HITS: u64 = 424_818_428_837_515_450;

/// The remedy's point of a ray from `start` straight up, past the unit square, which it misses.
fn aimed(start: [f64; 3]) -> Option<[f64; 3]> {
    let (snapshot, _) = model(&[square()]);
    let report = hit(&ready(&Exec::serial(), &snapshot), &ray(start, [0.0, 0.0, 1.0]), 0.0).unwrap_err();
    report.remedy(RemedyKind::AimAt).and_then(|remedy| match remedy.value("point")?.value {
        occam_report::Value::Point(point) => Some(point),
        occam_report::Value::Scalar(_) => None,
    })
}

#[test]
fn a_ray_that_misses_is_aimed_at_the_centre_nearest_its_way_and_where_two_tie_at_the_first() {
    // The square is two triangles either side of its diagonal from (0, 0) to (1, 1), the first below it: their
    // centres are at (2/3, 1/3) and (1/3, 2/3). Straight up from above the square, the way to the farther centre is
    // the nearer the ray's.
    assert_eq!(aimed([0.9, 0.1, 3.0]), Some([1.0 / 3.0, 2.0 / 3.0, 0.0]), "the second triangle's centre, the farther");
    assert_eq!(aimed([0.2, 0.7, 3.0]), Some([2.0 / 3.0, 1.0 / 3.0, 0.0]), "the first's");
    assert_eq!(aimed([0.5, 0.5, 3.0]), Some([2.0 / 3.0, 1.0 / 3.0, 0.0]), "as far from both: the first");
    assert_eq!(
        aimed([5.0, 0.5, 0.0]),
        None,
        "from a start in their plane, a ray towards either centre crosses neither"
    );
}

#[test]
fn an_edge_level_with_the_face_hit_within_the_tolerance_wins_though_another_face_owns_it() {
    // A disc of radius 0.5 in the square's plane: down 0.002 outside its rim, the ray hits the square, 0.36 from the
    // square's own edges, and passes the disc's rim, which is no edge of the square, within the tolerance there.
    let (snapshot, edges) = model(&[plane_and_cylinder(2.0), disc_drawn(0.5)]);
    let picker = ready(&Exec::serial(), &snapshot);
    let down = ray([0.355, 0.355, 5.0], [0.0, 0.0, -1.0]);
    assert_eq!(picker.first_face(&down), Some((5.0, 0)), "the square, 5 along");
    assert_eq!(picked(&picker, &down, 0.01), edges[1][0].entity(), "the disc's rim");
    assert_eq!(picked(&picker, &down, 0.001), snapshot.ids::<Face>()[0].entity(), "out of reach: the square");
}

#[test]
fn a_segment_comes_nearest_a_ray_where_the_two_are_square_and_a_ray_along_it_at_its_start() {
    let (a, b) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 6.0, 3.0));
    let across = approach(&ray([2.5, 4.0, 10.0], [0.0, 0.0, -1.0]), [a, b], (7, 2));
    assert_eq!(across, Near { edge: 7, sample: 2, apart: 0.0, along: 7.0, point: Vec3::new(2.5, 4.0, 3.0) });
    let beyond = approach(&ray([7.0, 10.0, 5.0], [0.0, 0.0, -1.0]), [a, b], (0, 0));
    assert_eq!((beyond.point, beyond.along, beyond.apart), (b, 2.0, 5.0), "past its end, at its end");
    // Along the segment's line, a millionth of a degree off it: one millimetre across, the two never come square.
    let (from, to) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
    let along = ray([-1.0, -0.01, 0.0], [1.0, 1e-9, 0.0]);
    let start = approach(&along, [from, to], (0, 0));
    assert!(start.along < 1.5 && start.point.x < 1e-9, "at its start, not its end: {start:?}");
    let none = approach(&ray([0.0, 3.0, 4.0], [1.0, 0.0, 0.0]), [from, from], (0, 0));
    assert_eq!((none.point, none.along, none.apart), (from, 0.0, 5.0), "a segment of no length is its start");
}

#[test]
fn a_point_is_as_far_from_a_segment_as_from_its_nearest_point_on_it() {
    let (a, b) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 6.0, 3.0));
    let away = Vec3::new(1.6, -1.2, 0.0);
    for (p, at) in [
        (a + (b - a) * 0.5, "its middle"),
        (b + Vec3::new(1.2, 1.6, 0.0), "past b"),
        (a - Vec3::new(1.2, 1.6, 0.0), "before a"),
    ] {
        let off = if at == "its middle" { p + away } else { p };
        assert!((apart(off, a, b) - 2.0).abs() < 1e-12, "{at}: {}", apart(off, a, b));
    }
    assert_eq!(apart(Vec3::new(1.0, 2.0, 8.0), a, a), 5.0, "from a segment of no length, its start");
}
