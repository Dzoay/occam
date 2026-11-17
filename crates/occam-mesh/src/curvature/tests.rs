//! Source: original

use core::f64::consts::{FRAC_PI_2, PI};

use occam_geom::{Cylinder, Jet1, Plane, Sphere, SplineSurface};
use occam_math::larger;
use occam_units::{Length, Placement, Tangent, Vec3};

use super::*;
use crate::testing::{collapsed_poles, disc, model, plane_and_cylinder, tolerance};

fn patch(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
    UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1))
}

fn band() -> f64 {
    tolerance().band().millimetres()
}

#[test]
fn a_sphere_bends_by_the_inverse_of_its_radius_its_poles_unread() {
    let sphere = Sphere::placed(&Placement::identity(), Length::from_millimetres(2.0)).unwrap();
    let bend = largest(&sphere, patch((0.0, -FRAC_PI_2), (PI, FRAC_PI_2)), band());
    // At an umbilic H^2 - K is rounding, whose square root is near 1e-8 of the curvature.
    assert!((bend - 0.5).abs() < 1e-7, "{bend}");
}

#[test]
fn a_cylinder_bends_by_the_inverse_of_its_radius_and_a_plane_not_at_all() {
    let cylinder = Cylinder::placed(&Placement::identity(), Length::from_millimetres(4.0)).unwrap();
    let bend = largest(&cylinder, patch((0.0, 0.0), (PI, 3.0)), band());
    assert!((bend - 0.25).abs() < 1e-7, "{bend}");
    assert_eq!(largest(&Plane::placed(&Placement::identity()), patch((0.0, 0.0), (1.0, 1.0)), band()), 0.0);
}

/// The largest curvature read on the grid's points along `v = 0`, where P9's patch collapses.
fn on_the_row(surface: &dyn Surface) -> f64 {
    let row = grid(patch((0.0, 0.0), (1.0, 1.0))).into_iter().take(GRID);
    row.map(|uv| principal(&surface.second(uv))).fold(0.0, larger)
}

#[test]
fn no_estimate_is_read_on_a_collapsed_row_of_p9s_bicubic_patch() {
    let poles = collapsed_poles();
    let surface = SplineSurface::bezier((4, 4), poles.clone(), None).unwrap();
    let bend = largest(&surface, patch((0.0, 0.0), (1.0, 1.0)), band());
    assert!(bend > 0.5 && bend < 50.0, "{bend}");
    // The same patch with its parameters swapped, collapsed along `u = 0`, bends the same.
    let swapped: Vec<Point<World>> =
        (0..4).flat_map(|j| (0..4).map(move |i| (i, j))).map(|(i, j)| poles[i * 4 + j]).collect();
    let turned = SplineSurface::bezier((4, 4), swapped, None).unwrap();
    let across = largest(&turned, patch((0.0, 0.0), (1.0, 1.0)), band());
    assert!((across - bend).abs() <= 1e-12 * bend, "{across} against {bend}");
    // Read there, the row's rounding noise would have been taken for a curvature far beyond any the patch has.
    assert!(on_the_row(&surface) > 1e6, "{}", on_the_row(&surface));
}

#[test]
fn a_face_reads_its_box_from_its_edges_curves_in_its_parameters() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), disc(1.0)]);
    let bends: Vec<f64> = snapshot.ids::<Face>().into_iter().map(|f| of_face(&snapshot, f)).collect();
    assert_eq!(bends.len(), 3);
    assert_eq!(bends[0], 0.0, "the plane");
    assert!((bends[1] - 0.5).abs() < 1e-7, "the cylinder: {}", bends[1]);
    assert_eq!(bends[2], 0.0, "the disc, whose edge has no curve in its parameters");
}

/// A jet at the origin with these first and second derivatives, the second all along +Z.
fn jet(du: [f64; 3], dv: [f64; 3], [uu, uv, vv]: [f64; 3]) -> Jet2 {
    let rate = |[x, y, z]: [f64; 3]| Tangent::new(Vec3::new(x, y, z));
    let first = Jet1 { point: Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), du: rate(du), dv: rate(dv) };
    Jet2 { first, duu: rate([0.0, 0.0, uu]), duv: rate([0.0, 0.0, uv]), dvv: rate([0.0, 0.0, vv]) }
}

#[test]
fn the_principal_curvatures_come_from_both_fundamental_forms_however_skewed_the_parameters() {
    // x = u + v and y = v, so z = (u^2 + uv + 2 v^2) / 2 has the Hessian [[1, -0.5], [-0.5, 2]] in x and y, whose
    // eigenvalues are (3 +- sqrt 2) / 2.
    let skewed = principal(&jet([1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.5, 2.0]));
    assert!((skewed - (1.5 + 0.5 * 2.0_f64.sqrt())).abs() < 1e-7, "{skewed}");
    let under = principal(&jet([1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [-1.0, -0.5, -2.0]));
    assert!((under - skewed).abs() < 1e-12, "bending away from the normal is as sharp: {under}");
    // A saddle: H is 0 and K is -1.
    assert_eq!(principal(&jet([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, -1.0])), 1.0);
    assert!(principal(&jet([1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 0.0, 1.0])).is_nan(), "no normal");
}

#[test]
fn the_grid_is_eight_points_each_way_its_ends_included() {
    let points = grid(patch((0.0, 0.0), (7.0, 14.0)));
    let expected: Vec<UvPoint> =
        (0..8).flat_map(|j| (0..8).map(move |i| UvPoint::new(f64::from(i), 2.0 * f64::from(j)))).collect();
    assert_eq!(points, expected);
}

#[test]
fn a_box_holds_both_boxes() {
    let joined = joined(patch((0.0, 1.0), (2.0, 3.0)), patch((-1.0, 2.0), (1.0, 4.0)));
    assert_eq!((joined.lo, joined.hi), (UvPoint::new(-1.0, 1.0), UvPoint::new(2.0, 4.0)));
}
