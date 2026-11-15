//! Source: ericson-2005
//! How far a surface strays from a face's triangles, measured to the triangle (P9: measured to the parametrically
//! corresponding point instead, uneven parameters counted as error, and refinement chased it into slivers). The
//! closest point of a triangle to a point is found by which of the triangle's regions the point projects into
//! (Ericson, 5.1.5): a corner, an edge or the inside. The regions meet where their closest points do, so where a
//! rounding puts a point on one side of a border or the other moves the answer by a rounding at most.

use occam_geom::Surface;
use occam_units::{Point, UvPoint, Vec3};

use crate::faces::FaceMesh;
use crate::limits::{higher, lower};

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

/// How far `p` is from the triangle `a`, `b`, `c`: from its nearest corner where the triangle has no area to speak
/// of, so that the region test's divisions have nothing to divide by.
pub(crate) fn distance(p: Vec3, [a, b, c]: [Vec3; 3]) -> f64 {
    let off = (p - closest(p, a, b, c)).norm();
    if off.is_finite() {
        return off;
    }
    [a, b, c].into_iter().map(|corner| (p - corner).norm()).fold(f64::INFINITY, lower)
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
            furthest = higher(furthest, distance(surface.point(between(corners, at, STEPS)).to_vec3(), flat));
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
    fn a_triangle_with_no_area_is_measured_from_its_nearest_corner() {
        assert_eq!(distance(Vec3::new(1.0, 1.0, 0.0), [A, A, A]), 2.0_f64.sqrt());
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
