//! Source: ericson-2005
//! How far a surface strays from a face's triangles, measured to the triangle (P9: measured to the parametrically
//! corresponding point instead, uneven parameters counted as error, and refinement chased it into slivers). The
//! closest point of a triangle to a point is found by which of the triangle's regions the point projects into
//! (Ericson, 5.1.5): a corner, an edge or the inside. The regions meet where their closest points do, so where a
//! rounding puts a point on one side of a border or the other moves the answer by a rounding at most.

use occam_geom::Surface;
use occam_math::{larger, smaller};
use occam_units::{Point, UvPoint, Vec3};

use crate::faces::FaceMesh;

/// The point of the triangle `a`, `b`, `c` closest to `p`.
fn closest(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let whole = va + vb + vc;
    a + ab * (vb / whole) + ac * (vc / whole)
}

/// How thin a triangle may be, as twice its area over its longest side squared, for the region test: 2^-13. Its dot
/// products are of the size of the longest side to the fourth, and their differences of twice the area squared, so a
/// thinner triangle's are lost to rounding (the catalogue's sliver walls: 20 long and 1e-5 high, a point on one was
/// read as 0.28 from it). At this ratio they are good to 2^-26 of themselves.
const THIN: f64 = 1.0 / 8192.0;

/// Whether the triangle `a`, `b`, `c` is thick enough for the region test: twice its area more than [`THIN`] times its
/// longest side squared. One with no area, or whose numbers are not, is not.
fn thick(a: Vec3, b: Vec3, c: Vec3) -> bool {
    let twice = (b - a).cross(c - a).norm();
    let longest = larger(larger((b - a).dot(b - a), (c - b).dot(c - b)), (a - c).dot(a - c));
    twice > THIN * longest
}

/// How far `p` is from the triangle `a`, `b`, `c`: by the region test where it is [`thick`], and otherwise by
/// [`sliver`].
pub(crate) fn distance(p: Vec3, [a, b, c]: [Vec3; 3]) -> f64 {
    if thick(a, b, c) {
        return (p - closest(p, a, b, c)).norm();
    }
    sliver(p, [a, b, c])
}

/// How far `p` is from the triangle `a`, `b`, `c`, read through cross products, which keep a thin triangle's area: from
/// its plane where `p` lies over it, each corner pair turning about `p` as the triangle does, and otherwise from the
/// nearest of its sides; from its nearest side where it has no area at all.
fn sliver(p: Vec3, [a, b, c]: [Vec3; 3]) -> f64 {
    let normal = (b - a).cross(c - a);
    let over = [(a, b), (b, c), (c, a)].into_iter().all(|(s, t)| (s - p).cross(t - p).dot(normal) >= 0.0);
    let height = ((p - a).dot(normal) / normal.norm()).abs();
    if over && height.is_finite() {
        return height;
    }
    [(a, b), (b, c), (c, a)].into_iter().map(|(s, t)| to_segment(p, s, t)).fold(f64::INFINITY, smaller)
}

/// How far `p` is from the segment from `a` to `b`: from `a` itself where they are one point.
pub(crate) fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let along = b - a;
    let at = larger(0.0, smaller(1.0, (p - a).dot(along) / along.dot(along)));
    (p - (a + along * at)).norm()
}

/// The point a fraction `(i, j, k) / n` of the way to each of `corners`, in a face's parameters.
#[expect(clippy::many_single_char_names, reason = "the corners and their weights")]
pub(crate) fn between(corners: [UvPoint; 3], [i, j, k]: [u32; 3], n: u32) -> UvPoint {
    let [a, b, c] = corners;
    let (wi, wj, wk, whole) = (f64::from(i), f64::from(j), f64::from(k), f64::from(n));
    UvPoint::new((wi * a.u() + wj * b.u() + wk * c.u()) / whole, (wi * a.v() + wj * b.v() + wk * c.v()) / whole)
}

/// The points of a grid of `n` steps a side on a triangle, in fractions of `n` of the way to each corner, the
/// corners and edges included.
pub(crate) fn grid(n: u32) -> impl Iterator<Item = [u32; 3]> {
    (0..=n).flat_map(move |i| (0..=n.saturating_sub(i)).map(move |j| [i, j, n.saturating_sub(i).saturating_sub(j)]))
}

/// How many steps a side the grid [`deviation`] reads has: more than refinement reads, and most of its points others,
/// so that it checks refinement rather than repeating it.
const STEPS: u32 = 8;

/// The furthest `surface` strays from any triangle of `mesh`, the face's mesh on it, measured to the triangle: at
/// the points of a grid of 8 steps a side on each triangle in the face's parameters, 45 of them, so a lower bound on
/// the true furthest, as any sampling is (P9 read 28).
#[must_use]
pub fn deviation(mesh: &FaceMesh, surface: &dyn Surface) -> f64 {
    let vertex = |k: u32| usize::try_from(k).ok().and_then(|k| Some((*mesh.points.get(k)?, *mesh.uvs.get(k)?)));
    let mut furthest = 0.0;
    for triangle in &mesh.triangles {
        let [Some(a), Some(b), Some(c)] = triangle.map(vertex) else { continue };
        let (corners, flat) = ([a.1, b.1, c.1], [a.0, b.0, c.0].map(Point::to_vec3));
        for at in grid(STEPS) {
            furthest = larger(furthest, distance(surface.point(between(corners, at, STEPS)).to_vec3(), flat));
        }
    }
    furthest
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    const B: Vec3 = Vec3 { x: 2.0, y: 0.0, z: 0.0 };
    const C: Vec3 = Vec3 { x: 0.0, y: 2.0, z: 0.0 };

    #[test]
    fn a_point_over_the_inside_is_as_far_as_its_height_and_one_beyond_a_corner_or_an_edge_from_it() {
        assert_eq!(distance(Vec3::new(0.5, 0.5, 3.0), [A, B, C]), 3.0);
        assert_eq!(distance(Vec3::new(-3.0, -4.0, 0.0), [A, B, C]), 5.0, "beyond A");
        assert_eq!(distance(Vec3::new(5.0, -4.0, 0.0), [A, B, C]), 5.0, "beyond B");
        assert_eq!(distance(Vec3::new(-1.0, 6.0, 0.0), [A, B, C]), 17.0_f64.sqrt(), "beyond C");
        assert_eq!(distance(Vec3::new(1.0, -2.0, 0.0), [A, B, C]), 2.0, "beyond AB");
        assert_eq!(distance(Vec3::new(-2.0, 1.0, 0.0), [A, B, C]), 2.0, "beyond CA");
        assert_eq!(distance(Vec3::new(2.0, 2.0, 0.0), [A, B, C]), 2.0_f64.sqrt(), "beyond BC");
    }

    #[test]
    fn a_point_beyond_the_edge_opposite_an_obtuse_corner_is_measured_to_that_edge() {
        // Each of these lies beyond the edge from the second corner to the third, and the region test takes it there,
        // not into the triangle.
        let at = |x: f64, y: f64| Vec3::new(x, y, 0.0);
        let near = distance(at(0.5, 1.0), [at(-3.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)]);
        assert!((near - 0.125_f64.sqrt()).abs() < 1e-15, "{near}");
        assert_eq!(distance(at(-0.5, -3.0), [at(0.0, 1.0), at(-1.0, 0.0), at(3.0, 0.0)]), 3.0);
    }

    #[test]
    fn a_point_on_a_sliver_is_no_distance_from_it_and_one_beside_it_as_far_as_it_is() {
        // A wall 20 long and from 1e-5 to 1e-11 high, as P9's sliver walls are triangulated: in the dot products of
        // the region test, of size 400, the wall's area squared is lost to rounding.
        for height in [1e-5, 1e-7, 1e-9, 1e-11] {
            let corners = [Vec3::new(40.0, 0.0, 0.0), Vec3::new(40.0, 20.0, 0.0), Vec3::new(40.0, 20.0, height)];
            let on = |[i, j, k]: [u32; 3]| {
                let [a, b, c] = corners;
                (a * f64::from(i) + b * f64::from(j) + c * f64::from(k)) * (1.0 / 8.0)
            };
            for at in grid(8) {
                assert!(distance(on(at), corners) < 1e-12, "{height}: {at:?}");
                let beside = on(at) + Vec3::new(3e-3, 0.0, 0.0);
                assert!((distance(beside, corners) - 3e-3).abs() < 1e-12, "{height}: {at:?}");
            }
            assert!((distance(Vec3::new(40.0, 10.0, -1e-3), corners) - 1e-3).abs() < 1e-12, "{height}: under it");
            assert!((distance(Vec3::new(40.0, 25.0, 0.0), corners) - 5.0).abs() < 1e-12, "{height}: past its end");
        }
    }

    #[test]
    fn a_triangle_is_thick_where_twice_its_area_is_more_than_2_to_the_minus_13_of_its_longest_side_squared() {
        // Exactly at the bound, in exact numbers: twice the area is 4 * 2^-11, and the longest side squared 16.
        let tip = 1.0 / 2048.0;
        assert!(!thick(Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(2.0, tip, 0.0)), "at the bound");
        assert!(!thick(A, A, B), "no area");
        assert!(!thick(A, B, Vec3::new(f64::NAN, 0.0, 0.0)), "not a number");
        // A side of 4 along `along` and an apex `h` off it along `off`, tilted so that every side has a part along
        // every axis, and moved 1000 either way along each: a side's length read as anything else, as `|b|^2 - |a|^2`,
        // is thousands where it should be 16, or below the next side's.
        let (along, off) = (Vec3::new(2.0, 1.0, 2.0) * (1.0 / 3.0), Vec3::new(1.0, 2.0, -2.0) * (1.0 / 3.0));
        let away = [1000.0, -1000.0];
        let offsets =
            away.into_iter().flat_map(|k| [Vec3::new(k, 0.0, 0.0), Vec3::new(0.0, k, 0.0), Vec3::new(0.0, 0.0, k)]);
        for at in offsets.chain([Vec3::new(0.0, 0.0, 0.0)]) {
            let triangle = |h: f64| (at, at + along * 4.0, at + along * 2.0 + off * h);
            let (a, b, c) = triangle(4.0 / 4096.0);
            assert!(thick(a, b, c), "twice the area 2^-12 of the longest squared, at {at:?}");
            assert!(thick(b, c, a) && thick(c, a, b), "whichever corner comes first, at {at:?}");
            let (a, b, c) = triangle(4.0 / 16384.0);
            assert!(!thick(a, b, c), "2^-14, at {at:?}");
            assert!(!thick(b, c, a) && !thick(c, a, b), "whichever corner comes first, at {at:?}");
        }
    }

    #[test]
    fn a_triangle_with_no_area_is_measured_from_its_nearest_side() {
        assert_eq!(distance(Vec3::new(1.0, 1.0, 0.0), [A, A, A]), 2.0_f64.sqrt());
        // Two corners at one place: a segment, and the point is as far from it as from the line it lies on.
        let (a, c) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0));
        assert_eq!(distance(Vec3::new(1.5, 1.0, 0.0), [a, a, c]), 1.0);
        assert_eq!(distance(Vec3::new(4.0, 0.0, 0.0), [a, c, a]), 1.0, "beyond its end");
    }

    #[test]
    fn a_mesh_strays_from_its_surface_as_far_as_the_surface_is_from_its_triangles() {
        use occam_geom::Plane;
        use occam_units::{Direction, Local, Placement, Vector, World};

        // A triangle in z = 0, against the plane z = 0.5 over the same (x, y).
        let up = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, 0.5)));
        let surface = Plane::placed(&Placement::<Local, World>::identity().then(&up));
        let corners = [(0.0, 0.0), (6.0, 0.0), (0.0, 6.0)];
        let mesh = FaceMesh {
            points: corners.map(|(x, y)| Point::from_vec3(Vec3::new(x, y, 0.0))).into(),
            normals: vec![Direction::normalised(Vec3::new(0.0, 0.0, 1.0)); 3],
            uvs: corners.map(|(u, v)| UvPoint::new(u, v)).into(),
            triangles: vec![[0, 1, 2]],
            sides: Vec::new(),
            capped: false,
        };
        assert_eq!(deviation(&mesh, &surface), 0.5);
    }

    #[test]
    fn the_grid_holds_every_point_of_its_steps_once_and_each_point_is_between_its_corners() {
        let points: Vec<[u32; 3]> = grid(6).collect();
        assert_eq!(points.len(), 28);
        assert!(points.iter().all(|p| p.iter().sum::<u32>() == 6));
        let corners = [UvPoint::new(0.0, 0.0), UvPoint::new(6.0, 0.0), UvPoint::new(0.0, 12.0)];
        let middle = between(corners, [2, 2, 2], 6);
        assert_eq!((middle.u(), middle.v()), (2.0, 4.0));
    }
}
