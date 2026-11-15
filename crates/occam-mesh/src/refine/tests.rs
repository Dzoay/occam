//! Source: original

use core::f64::consts::FRAC_PI_2;

use occam_geom::{Cylinder, Plane};
use occam_math::cos;
use occam_units::{Angle, Direction, Length, Local, Placement, Vector, World};

use super::*;

/// The frame at the origin, moved by `then`.
fn placed(then: &Placement<World, World>) -> Placement<Local, World> {
    Placement::<Local, World>::identity().then(then)
}

/// The plane through `(0, 0, height)`, turned about the x axis by `tilt` so that it rises along `v`.
fn plane(height: f64, tilt: f64) -> Plane {
    let turn =
        Placement::<World, World>::rotation(Direction::normalised(Vec3::new(1.0, 0.0, 0.0)), Angle::from_radians(tilt));
    let up = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, height)));
    Plane::placed(&placed(&turn).then(&up))
}

/// What refinement holds triangles to on `surface`: `deflection`, and the angle whose half has the cosine `turn`.
fn work(surface: &dyn Surface, deflection: f64, turn: f64, floor: f64) -> Work<'_> {
    Work { surface, deflection, turn, floor, square: 1e-9, cap: usize::MAX, scale: 1.0 }
}

/// The triangle from `(0, 0)` to `(6, 0)` to `(0, 6)`, in the parameters and in the plane z = 0 alike, its corners
/// turned on by `by`.
fn corners(by: usize) -> ([UvPoint; 3], [Vec3; 3]) {
    let mut at = [(0.0, 0.0), (6.0, 0.0), (0.0, 6.0)];
    at.rotate_left(by);
    (at.map(|(u, v)| UvPoint::new(u, v)), at.map(|(x, y)| Vec3::new(x, y, 0.0)))
}

/// `wants`' answer as numbers.
fn wanted(work: &Work<'_>, (uv, at): ([UvPoint; 3], [Vec3; 3]), fixed: [bool; 3]) -> Option<(f64, f64)> {
    wants(work, uv, at, fixed).map(|uv| (uv.u(), uv.v()))
}

#[test]
fn a_triangle_exactly_at_its_share_of_the_deflection_is_kept_and_one_beyond_it_takes_the_first_point_that_strays_most()
{
    // 15/16 above every point of the triangle: exactly its share of a deflection of 1, everywhere alike.
    let surface = plane(0.9375, 0.0);
    assert_eq!(wanted(&work(&surface, 1.0, -1.0, 0.0), corners(0), [true; 3]), None);
    // At half that deflection every point inside strays as far, and the first of them in the grid's order is taken.
    assert_eq!(wanted(&work(&surface, 0.5, -1.0, 0.0), corners(0), [true; 3]), Some((1.0, 4.0)));
}

#[test]
fn the_point_taken_is_the_one_inside_that_strays_most() {
    // The plane rises along `v`, so the point inside with the greatest `v` strays most.
    let surface = plane(0.25, 0.1);
    assert_eq!(wanted(&work(&surface, 0.5, -1.0, 0.0), corners(0), [true; 3]), Some((1.0, 4.0)));
    let (uv, at) = corners(1);
    assert_eq!(wanted(&work(&surface, 0.5, -1.0, 0.0), (uv, at), [true; 3]), Some((1.0, 4.0)), "turned");
}

#[test]
fn points_on_a_fixed_edge_and_corners_are_not_read_whichever_corner_the_surface_rises_to() {
    // A plane rising along `v` by sin 0.1, near 0.1, a millimetre: near 0.4 above the highest point inside, 0.5 above
    // the highest points of the two edges at (0, 6) and 0.6 above that corner. 0.45 lies between.
    let surface = plane(0.0, 0.1);
    let held = work(&surface, 0.48, -1.0, 0.0);
    for by in 0..3 {
        assert_eq!(wanted(&held, corners(by), [true; 3]), None, "turned by {by}");
        assert!(wanted(&held, corners(by), [false; 3]).is_some(), "turned by {by}, its edges free");
    }
}

#[test]
fn a_triangle_with_two_corners_at_one_place_is_never_refined() {
    let surface = plane(0.9375, 0.0);
    let (uv, [a, b, c]) = corners(0);
    for at in [[a, a, c], [a, b, b], [a, b, a]] {
        assert_eq!(wants(&work(&surface, 0.5, -1.0, 0.0), uv, at, [true; 3]), None, "{at:?}");
    }
}

#[test]
fn the_normal_is_held_to_half_the_angle_only_on_a_triangle_longer_than_twice_the_floor() {
    // A cylinder of radius 10 about the x axis, its normal along +z at u = 0, against a triangle 3, 4 and 5 long in a
    // plane facing +z. The triangle's middle is at u = 0, and its grid's points reach u = ±0.2, where the normal has
    // turned 0.2: four times half an angle of 0.1.
    let turn = Placement::<World, World>::rotation(
        Direction::normalised(Vec3::new(0.0, 1.0, 0.0)),
        Angle::from_radians(-FRAC_PI_2),
    );
    let surface = Cylinder::placed(&placed(&turn), Length::from_millimetres(10.0)).unwrap();
    let middle = normal(&surface, UvPoint::new(0.0, 1.0 / 3.0)).unwrap();
    assert!((middle - Vec3::new(0.0, 0.0, 1.0)).norm() < 1e-15, "{middle:?}");
    let uv = [UvPoint::new(-0.3, 0.0), UvPoint::new(0.3, 0.0), UvPoint::new(0.0, 1.0)];
    let at = [Vec3::new(1.0, 1.0, 0.0), Vec3::new(4.0, 1.0, 0.0), Vec3::new(1.0, 5.0, 0.0)];
    let half = cos(0.05);
    assert!(wants(&work(&surface, 1e12, half, 0.1), uv, at, [true; 3]).is_some(), "above the floor");
    assert_eq!(wants(&work(&surface, 1e12, half, 2.5), uv, at, [true; 3]), None, "exactly twice the floor long");
}

#[test]
fn a_flat_triangle_on_a_plane_is_never_refined_however_fine_the_angle() {
    let surface = plane(0.0, 0.0);
    assert_eq!(wanted(&work(&surface, 1e12, cos(0.01), 0.0), corners(0), [true; 3]), None);
}

#[test]
fn a_corner_is_not_read_even_where_it_strays_most() {
    // The triangle's corner at (0, 6) is lifted 0.6 off the plane z = 0 it is drawn on: the surface strays near 0.6
    // from the triangle there, near 0.5 at the points of the edges beside it, and less inside. Its edges are free, so
    // only the corner strays beyond 0.55.
    let surface = plane(0.0, 0.0);
    let held = work(&surface, 0.55 / 0.9375, -1.0, 0.0);
    for by in 0..3 {
        let (mut uv, mut at) = corners(0);
        at[2].z = 0.6;
        uv.rotate_left(by);
        at.rotate_left(by);
        assert_eq!(wants(&held, uv, at, [false; 3]), None, "turned by {by}");
    }
}

#[test]
fn a_small_triangle_facing_as_its_surface_does_is_not_turned_over() {
    // Its normal, 3.6e-11 long, lies along the plane's: far from a right angle, however small the triangle.
    let surface = plane(0.0, 0.0);
    let uv = [UvPoint::new(0.0, 0.0), UvPoint::new(6e-6, 0.0), UvPoint::new(0.0, 6e-6)];
    let at = uv.map(|p| Vec3::new(p.u(), p.v(), 0.0));
    assert_eq!(wants(&work(&surface, 1.0, -1.0, 0.0), uv, at, [true; 3]), None);
}
