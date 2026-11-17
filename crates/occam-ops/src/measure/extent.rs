//! Source: original
//! The tight axis-aligned box of any body (PARITY 2.7): the box of its points themselves, not of the boxes of the
//! surfaces and curves they lie on. A side of the box is the furthest the body reaches along an axis, which is on an edge
//! or at a point inside a face where the face's normal lies along the axis.
//!
//! - **An edge** gives its curve's own box over its range, which for a line, a circle or an ellipse is the arc's own,
//!   to sixteen roundings. A curve whose box does not close on it, as a spline's, the box of its control points over a
//!   span, holds it, not proved tight.
//! - **Inside a face,** where its surface is analytic: a plane, a cylinder and a cone have no point there that reaches
//!   further than all their edges, since where the normal lies along an axis they are straight across it, to the
//!   edges; a sphere reaches furthest at its centre plus and minus its radius along each axis, and a torus at its
//!   centre plus and minus, along each axis not its own, the tube's radius and its circle's reach that way. Each
//!   counts where it lies inside the face. A face on a surface with no analytic form gives its surface's box over the
//!   face's parameters, which holds it, not proved tight.

use occam_geom::{Analytic, Scratch, Surface, UvBox, Wrap};
use occam_topo::winding_number;
use occam_units::{Bounds, Point, Tolerance, UvPoint, Vec3, World};

use super::mass::Patch;

/// The world's axes.
const AXES: [Vec3; 3] = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];

/// The smaller of two numbers, the first if they are equal.
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The larger of two numbers, the first if they are equal.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The points of `surface`, analytic to `tolerance`, where it reaches furthest along an axis further than its edges
/// may, either way along it; `None` where it has no analytic form.
pub(super) fn turning(surface: &dyn Surface, tolerance: Tolerance) -> Option<Vec<Vec3>> {
    let mut points = Vec::new();
    match surface.analytic(tolerance)? {
        Analytic::Plane { .. } | Analytic::Cylinder { .. } | Analytic::Cone { .. } => {}
        Analytic::Sphere { centre, radius } => {
            let (c, r) = (centre.to_vec3(), radius.millimetres());
            for e in AXES {
                points.extend([c + e * r, c - e * r]);
            }
        }
        Analytic::Torus { centre, axis, major, minor } => {
            let (c, a, big, small) = (centre.to_vec3(), axis.get(), major.millimetres(), minor.millimetres());
            for e in AXES {
                // Along `e`, the torus reaches furthest where its normal is `e`: out from the axis across it, by the
                // circle's radius, and from the circle along `e`, by the tube's. Along its own axis, it reaches as far
                // round whole circles, which its edges reach too.
                let flat = e - a * e.dot(a);
                let across = flat.norm();
                if across > 1e-12 {
                    let reach = flat * (big / across) + e * small;
                    points.extend([c + reach, c - reach]);
                }
            }
        }
    }
    Some(points)
}

/// The value equal to `x` nearest `near`, where it comes round every `period`.
fn nearest(x: f64, near: f64, period: Option<f64>) -> f64 {
    period.map_or(x, |p| x + p * ((near - x) / p).round())
}

/// Each loop of `patch` as a chain of segments in its parameters, from its pieces' samples, and the box they span.
fn chains(patch: &Patch<'_>) -> (Vec<Vec<[UvPoint; 2]>>, UvBox) {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    let mut out = Vec::new();
    for pieces in &patch.loops {
        let points: Vec<UvPoint> = pieces.iter().flat_map(|p| p.samples()).collect();
        for p in &points {
            lo = [smaller(lo[0], p.u()), smaller(lo[1], p.v())];
            hi = [larger(hi[0], p.u()), larger(hi[1], p.v())];
        }
        let next = points.iter().skip(1).chain(points.first());
        out.push(points.iter().zip(next).map(|(&a, &b)| [a, b]).collect());
    }
    (out, UvBox::new(UvPoint::new(lo[0], lo[1]), UvPoint::new(hi[0], hi[1])))
}

/// Whether `uv` lies in the region `loops` bound: inside the first, the outer, and inside none of the others, its holes.
/// A point on a loop is in it.
fn inside(loops: &[Vec<[UvPoint; 2]>], uv: UvPoint) -> bool {
    let within = |chain: &Vec<[UvPoint; 2]>| winding_number(chain, uv).is_none_or(|w| w != 0);
    let on = |chain: &Vec<[UvPoint; 2]>| winding_number(chain, uv).is_none();
    let mut each = loops.iter();
    each.next().is_some_and(within) && each.all(|hole| on(hole) || !within(hole))
}

/// What inside a face may reach further than its edges.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Inside {
    /// These points, each inside it.
    Points(Vec<Vec3>),
    /// Anything in this box, its surface's over the face's parameters, where the surface has no analytic form.
    Boxed(Bounds),
}

/// What inside `patch`'s face may reach further than its edges, of the points where its surface `turning`, as
/// [`turning`] finds them.
pub(super) fn interior(patch: &Patch<'_>, turning: Option<Vec<Vec3>>, tolerance: Tolerance) -> Inside {
    let (loops, span) = chains(patch);
    let Some(points) = turning else {
        return Inside::Boxed(patch.surface.bounds(span));
    };
    let domain = patch.surface.domain();
    let period = |range: occam_geom::ParamRange| (range.wrap == Wrap::Periodic).then_some(range.hi - range.lo);
    let middle = (0.5 * span.lo.u() + 0.5 * span.hi.u(), 0.5 * span.lo.v() + 0.5 * span.hi.v());
    let mut scratch = Scratch::default();
    let mut found = Vec::new();
    for p in points {
        let Ok(uv) = patch.surface.invert(Point::<World>::from_vec3(p), None, tolerance, &mut scratch) else {
            continue;
        };
        let placed =
            UvPoint::new(nearest(uv.u(), middle.0, period(domain.u)), nearest(uv.v(), middle.1, period(domain.v)));
        if inside(&loops, placed) {
            found.push(p);
        }
    }
    Inside::Points(found)
}

#[cfg(test)]
mod tests {
    use core::f64::consts::{PI, TAU};
    use occam_fixtures::point as at;

    use occam_geom::{Ellipse, Extrusion, Line, UvCurve};
    use occam_units::{Direction, Length, Local, Placement};

    use super::*;
    use crate::measure::mass::Piece;
    use crate::request::default_tolerance;

    #[test]
    fn a_sphere_and_a_torus_reach_furthest_at_their_centres_plus_and_minus_along_each_axis() {
        let tolerance = default_tolerance().unwrap();
        let shift = Placement::<World, World>::translation(occam_units::Vector::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
        let placement = Placement::<Local, World>::identity().then(&shift);
        let sphere = occam_geom::Sphere::placed(&placement, Length::from_millimetres(2.0)).unwrap();
        let along = |x: f64, y: f64, z: f64| Vec3::new(1.0 + x, 2.0 + y, 3.0 + z);
        let six =
            [(2.0, 0.0, 0.0), (-2.0, 0.0, 0.0), (0.0, 2.0, 0.0), (0.0, -2.0, 0.0), (0.0, 0.0, 2.0), (0.0, 0.0, -2.0)];
        assert_eq!(turning(&sphere, tolerance), Some(six.map(|(x, y, z)| along(x, y, z)).to_vec()));
        // A torus about z, its circle of radius 5 and tube of 1: out along x and y by 6, and not along z, its own axis.
        let torus = occam_geom::Torus::placed(&placement, Length::from_millimetres(5.0), Length::from_millimetres(1.0));
        let four = [(6.0, 0.0, 0.0), (-6.0, 0.0, 0.0), (0.0, 6.0, 0.0), (0.0, -6.0, 0.0)];
        let found = turning(&torus.unwrap(), tolerance).unwrap();
        assert_eq!(found.len(), 4);
        for (p, (x, y, z)) in found.iter().zip(four) {
            assert!((*p - along(x, y, z)).norm() < 1e-12, "{found:?}");
        }
        // Turned a quarter about x, its axis is along -y: it reaches 6 along x and z, and the tube's 1 along y only on
        // whole circles, which its edges reach.
        let quarter = Placement::<World, World>::rotation(
            Direction::normalised(Vec3::new(1.0, 0.0, 0.0)),
            occam_units::Angle::from_radians(0.5 * PI),
        );
        let turned = Placement::<Local, World>::identity().then(&quarter);
        let torus = occam_geom::Torus::placed(&turned, Length::from_millimetres(5.0), Length::from_millimetres(1.0));
        let found = turning(&torus.unwrap(), tolerance).unwrap();
        let want =
            [Vec3::new(6.0, 0.0, 0.0), Vec3::new(-6.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 6.0), Vec3::new(0.0, 0.0, -6.0)];
        assert!(found.iter().zip(want).all(|(p, w)| (*p - w).norm() < 1e-12) && found.len() == 4, "{found:?}");
        let plane = occam_geom::Plane::placed(&placement);
        assert_eq!(turning(&plane, tolerance), Some(Vec::new()), "a plane reaches no further inside than its edges");
    }

    #[test]
    fn a_point_inside_a_face_whose_parameters_come_round_again_is_found_where_the_face_is() {
        // A unit sphere's face over u from a turn and a half to a turn and two, a turn on from where the sphere's own
        // parameters put its points: it holds the point furthest along y, at a quarter turn, and no other.
        let sphere = occam_geom::Sphere::placed(&Placement::<Local, World>::identity(), Length::from_millimetres(1.0));
        let sphere = sphere.unwrap();
        let outer = ring(&[(TAU + 0.5, -0.5), (TAU + 2.0, -0.5), (TAU + 2.0, 0.5), (TAU + 0.5, 0.5)]);
        let patch = Patch { surface: &sphere, reversed: false, loops: vec![pieces(&outer)] };
        let tolerance = default_tolerance().unwrap();
        let found = interior(&patch, turning(&sphere, tolerance), tolerance);
        let Inside::Points(points) = found else { panic!("{found:?}") };
        assert_eq!(points.len(), 1, "{points:?}");
        assert!((points[0] - Vec3::new(0.0, 1.0, 0.0)).norm() < 1e-12, "{points:?}");
        // The same face a turn back, where the sphere's own parameters are: the same point.
        let back = ring(&[(0.5, -0.5), (2.0, -0.5), (2.0, 0.5), (0.5, 0.5)]);
        let patch = Patch { surface: &sphere, reversed: false, loops: vec![pieces(&back)] };
        assert_eq!(interior(&patch, turning(&sphere, tolerance), tolerance), Inside::Points(points));
    }

    #[test]
    fn a_point_inside_a_face_whose_tube_angle_comes_round_again_is_found_where_the_face_is() {
        // A torus of radii 5 and 1 turned an eighth about x, its axis now (0, -1, 1) / sqrt 2: it reaches least along y
        // and most along z at a tube angle of an eighth of a turn, inside a face over that angle a turn on; most along
        // y and least along z at minus that, and along x, in its circle's plane, at 0, all outside.
        let eighth = Placement::<World, World>::rotation(
            Direction::normalised(Vec3::new(1.0, 0.0, 0.0)),
            occam_units::Angle::from_radians(0.25 * PI),
        );
        let placement = Placement::<Local, World>::identity().then(&eighth);
        let torus = occam_geom::Torus::placed(&placement, Length::from_millimetres(5.0), Length::from_millimetres(1.0));
        let torus = torus.unwrap();
        let tolerance = default_tolerance().unwrap();
        let all = turning(&torus, tolerance).unwrap();
        let outer = ring(&[(0.01, TAU + 0.3), (TAU - 0.01, TAU + 0.3), (TAU - 0.01, TAU + 1.3), (0.01, TAU + 1.3)]);
        let patch = Patch { surface: &torus, reversed: false, loops: vec![pieces(&outer)] };
        assert_eq!(interior(&patch, Some(all.clone()), tolerance), Inside::Points(vec![all[3], all[4]]), "{all:?}");
    }

    #[test]
    fn the_smaller_and_the_larger_of_two_numbers() {
        assert_eq!((smaller(1.0, 2.0), smaller(2.0, 1.0), larger(1.0, 2.0), larger(2.0, 1.0)), (1.0, 1.0, 2.0, 2.0));
    }

    /// The straight curve in the parameters from `a` to `b`, and its length.
    fn segment(a: (f64, f64), b: (f64, f64)) -> (UvCurve<Line>, f64) {
        let along = Vec3::new(b.0 - a.0, b.1 - a.1, 0.0);
        (UvCurve::new(Line::new(at(a.0, a.1, 0.0), Direction::normalised(along))).unwrap(), along.norm())
    }

    /// The closed loop through `corners`.
    fn ring(corners: &[(f64, f64)]) -> Vec<(UvCurve<Line>, f64)> {
        corners.iter().zip(corners.iter().cycle().skip(1)).map(|(&a, &b)| segment(a, b)).collect()
    }

    fn pieces(curves: &[(UvCurve<Line>, f64)]) -> Vec<Piece<'_>> {
        curves.iter().map(|(c, length)| Piece::Curve(c as &dyn occam_geom::ParamCurve, 0.0, *length)).collect()
    }

    #[test]
    fn a_point_is_inside_a_region_in_its_outer_loop_and_out_of_its_holes_or_on_either() {
        let surface = occam_geom::Plane::placed(&Placement::<Local, World>::identity());
        let outer = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)]);
        let hole = ring(&[(1.0, 0.5), (1.0, 1.5), (2.0, 1.5), (2.0, 0.5)]);
        let patch = Patch { surface: &surface, reversed: false, loops: vec![pieces(&outer), pieces(&hole)] };
        let (loops, span) = chains(&patch);
        assert_eq!((span.lo, span.hi), (UvPoint::new(0.0, 0.0), UvPoint::new(4.0, 2.0)));
        let is = |u: f64, v: f64| inside(&loops, UvPoint::new(u, v));
        assert!(is(3.0, 1.0) && is(0.5, 1.0) && is(4.0, 1.0) && is(1.0, 1.0), "inside, or on a loop");
        assert!(!is(1.5, 1.0) && !is(5.0, 1.0) && !is(3.0, -1.0), "in the hole, or beyond");
        let none: Vec<Vec<[UvPoint; 2]>> = Vec::new();
        assert!(!inside(&none, UvPoint::new(1.0, 1.0)), "no loops, no region");
    }

    #[test]
    fn a_face_on_a_surface_with_no_analytic_form_gives_its_surfaces_box_over_its_parameters() {
        // An elliptic cylinder, 2 by 1 across and up z, over half a turn and 3 up: a box that holds it.
        let ellipse = Ellipse::placed(
            &Placement::<Local, World>::identity(),
            Length::from_millimetres(2.0),
            Length::from_millimetres(1.0),
        );
        let surface = Extrusion::new(ellipse.unwrap(), Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).unwrap();
        let tolerance = default_tolerance().unwrap();
        assert_eq!(turning(&surface, tolerance), None);
        let outer = ring(&[(0.0, 0.0), (PI, 0.0), (PI, 3.0), (0.0, 3.0)]);
        let patch = Patch { surface: &surface, reversed: false, loops: vec![pieces(&outer)] };
        let span = UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(PI, 3.0));
        assert_eq!(interior(&patch, None, tolerance), Inside::Boxed(surface.bounds(span)));
    }

    #[test]
    fn a_value_that_comes_round_is_taken_nearest_where_it_is_wanted() {
        assert_eq!(nearest(0.5, 7.0, Some(TAU)), 0.5 + TAU);
        assert_eq!(nearest(0.5, -5.0, Some(TAU)), 0.5 - TAU);
        assert_eq!(nearest(0.5, 7.0, None), 0.5);
        assert_eq!(nearest(-3.0, 2.0, Some(4.0)), 1.0);
    }
}
