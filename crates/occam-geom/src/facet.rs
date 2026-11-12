//! Source: ericson-2005
//! One triangle of a mesh-backed surface: its corners in the parameters and in space, and the affine map between them,
//! computed once (S4's cone), so that its point is `A + du (u - a.u) + dv (v - a.v)` and its first derivatives are
//! constant. Whether it holds a parameter is decided edge by edge in the direction of the edge's lower vertex index, so
//! that two triangles sharing an edge compute the same number for it and no point near it falls between them. The point
//! of it nearest a point in space is by Voronoi regions (Ericson, *Real-Time Collision Detection*, section 5.1.5).

use occam_units::{UvPoint, Vec3};

/// A triangle of a mesh, anticlockwise in the parameters.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Facet {
    /// The indices of its corners' vertices, which the surface's content names.
    pub(crate) corners: [u32; 3],
    /// Its corners in the parameters.
    pub(crate) uv: [UvPoint; 3],
    /// Its corners in space.
    pub(crate) points: [Vec3; 3],
    /// The rate of the point in `u`, the same all over it.
    du: Vec3,
    /// The rate of the point in `v`.
    dv: Vec3,
}

/// Twice the signed area of `a`, `b` and `p` in the parameters: above zero where `p` is to the left of `a` to `b`.
fn orient(a: UvPoint, b: UvPoint, p: UvPoint) -> f64 {
    (b.u() - a.u()) * (p.v() - a.v()) - (b.v() - a.v()) * (p.u() - a.u())
}

/// How far rounding can carry an edge's side at `uv`, which the edge from `from` to `to` gives: some machine epsilons
/// of the sizes in play, so that a parameter computed onto an edge, as a foot's is, is still held. It is at least the
/// rounding in [`orient`], whose two products and three differences are each within an epsilon, so the two triangles
/// on an edge, each computing its side from its own direction, cannot both put a point near it outside: that would need
/// their two roundings to differ by more than twice the slack.
fn slack(from: UvPoint, to: UvPoint, uv: UvPoint) -> f64 {
    let size = |p: UvPoint| p.u().abs() + p.v().abs();
    let edge = (to.u() - from.u()).abs() + (to.v() - from.v()).abs();
    8.0 * f64::EPSILON * edge * (size(from) + size(to) + size(uv))
}

/// The weights `(s, t)` on `b` and `c` of the point of the triangle `a`, `b`, `c` nearest `p`, by the region of its
/// plane that `p` projects into: a corner, an edge, or the face.
fn weights(point: Vec3, [first, second, third]: [Vec3; 3]) -> (f64, f64) {
    let (ab, ac, ap) = (second - first, third - first, point - first);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (0.0, 0.0);
    }
    let bp = point - second;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (1.0, 0.0);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (d1 / (d1 - d3), 0.0);
    }
    let cp = point - third;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (0.0, 1.0);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (0.0, d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let along = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (1.0 - along, along);
    }
    let whole = va + vb + vc;
    (vb / whole, vc / whole)
}

impl Facet {
    /// The triangle with these corners' indices, places in the parameters and points, or `None` where it does not run
    /// anticlockwise with some area in the parameters.
    pub(crate) fn new(corners: [u32; 3], uv: [UvPoint; 3], points: [Vec3; 3]) -> Option<Self> {
        let ([a, b, c], [pa, pb, pc]) = (uv, points);
        let det = orient(a, b, c);
        let anticlockwise = det > 0.0;
        if !anticlockwise {
            return None;
        }
        let (bu, bv, cu, cv) = (b.u() - a.u(), b.v() - a.v(), c.u() - a.u(), c.v() - a.v());
        let (to_b, to_c) = (pb - pa, pc - pa);
        let du = to_b * (cv / det) - to_c * (bv / det);
        let dv = to_c * (bu / det) - to_b * (cu / det);
        Some(Self { corners, uv, points, du, dv })
    }

    /// Whether `uv` is in it, edges included, or beyond an edge by no more than [`slack`]: so a point near an edge two
    /// triangles share is in one or both.
    pub(crate) fn holds(&self, uv: UvPoint) -> bool {
        let [a, b, c] = self.uv;
        [(a, b), (b, c), (c, a)].into_iter().all(|(from, to)| orient(from, to, uv) >= -slack(from, to, uv))
    }

    /// Whether the two overlap inside, in the parameters: no edge of either has every corner of the other on its line
    /// or beyond it, within [`slack`]. Two that share an edge or a corner do not.
    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        !(self.separates(other) || other.separates(self))
    }

    /// Whether an edge of this triangle has every corner of `other` on its line or beyond it, within [`slack`].
    fn separates(&self, other: &Self) -> bool {
        let [a, b, c] = self.uv;
        [(a, b), (b, c), (c, a)]
            .into_iter()
            .any(|(from, to)| other.uv.iter().all(|p| orient(from, to, *p) <= slack(from, to, *p)))
    }

    /// The point at `uv` and its derivatives in `u` and in `v`.
    pub(crate) fn jet(&self, uv: UvPoint) -> [Vec3; 3] {
        let ([a, ..], [origin, ..]) = (self.uv, self.points);
        [origin + self.du * (uv.u() - a.u()) + self.dv * (uv.v() - a.v()), self.du, self.dv]
    }

    /// The lowest and highest parameters of its corners.
    pub(crate) fn span(&self) -> (UvPoint, UvPoint) {
        let [a, b, c] = self.uv;
        let low = |x: f64, y: f64, z: f64| {
            if y < x {
                if z < y { z } else { y }
            } else if z < x {
                z
            } else {
                x
            }
        };
        let high = |x: f64, y: f64, z: f64| {
            if y > x {
                if z > y { z } else { y }
            } else if z > x {
                z
            } else {
                x
            }
        };
        (
            UvPoint::new(low(a.u(), b.u(), c.u()), low(a.v(), b.v(), c.v())),
            UvPoint::new(high(a.u(), b.u(), c.u()), high(a.v(), b.v(), c.v())),
        )
    }

    /// The parameters of its point nearest `p`, and how far that is, squared. A triangle collapsed to a line in space
    /// is nearest on an edge, by the regions of the edges; one whose face would need dividing by its zero area gives NaN,
    /// which a search passes over.
    pub(crate) fn nearest(&self, point: Vec3) -> (UvPoint, f64) {
        let [first, second, third] = self.points;
        let (on_second, on_third) = weights(point, self.points);
        let [ua, ub, uc] = self.uv;
        let uv = UvPoint::new(
            ua.u() + (ub.u() - ua.u()) * on_second + (uc.u() - ua.u()) * on_third,
            ua.v() + (ub.v() - ua.v()) * on_second + (uc.v() - ua.v()) * on_third,
        );
        let gap = first + (second - first) * on_second + (third - first) * on_third - point;
        (uv, gap.dot(gap))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(u: f64, v: f64) -> UvPoint {
        UvPoint::new(u, v)
    }

    fn facet() -> Facet {
        let points = [Vec3::new(1.0, 1.0, 0.0), Vec3::new(5.0, 1.0, 2.0), Vec3::new(1.0, 4.0, -1.0)];
        Facet::new([0, 1, 2], [at(0.0, 0.0), at(2.0, 0.0), at(0.0, 1.0)], points).unwrap()
    }

    #[test]
    fn the_map_takes_each_corner_to_its_point_and_has_constant_rates() {
        let f = facet();
        for (uv, point) in f.uv.into_iter().zip(f.points) {
            assert!((f.jet(uv)[0] - point).norm() < 1e-15, "{uv:?}");
        }
        let [_, du, dv] = f.jet(at(0.5, 0.25));
        assert_eq!((du, dv), (Vec3::new(2.0, 0.0, 1.0), Vec3::new(0.0, 3.0, -1.0)));
        assert_eq!(f.span(), (at(0.0, 0.0), at(2.0, 1.0)));
    }

    #[test]
    fn a_span_takes_each_extreme_from_whichever_corner_has_it() {
        let points = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        // Every corner order, and one whose u falls corner by corner, so each branch of the lowest and highest runs.
        for uv in [
            [at(1.0, 0.0), at(2.0, 1.0), at(0.0, 2.0)],
            [at(2.0, 1.0), at(0.0, 2.0), at(1.0, 0.0)],
            [at(0.0, 2.0), at(1.0, 0.0), at(2.0, 1.0)],
            [at(2.0, 1.0), at(1.0, 2.0), at(0.0, 0.0)],
        ] {
            assert_eq!(Facet::new([0, 1, 2], uv, points).unwrap().span(), (at(0.0, 0.0), at(2.0, 2.0)), "{uv:?}");
        }
    }

    #[test]
    fn triangles_overlap_inside_and_not_where_they_only_touch() {
        let points = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        let tri = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
            Facet::new([0, 1, 2], [at(a.0, a.1), at(b.0, b.1), at(c.0, c.1)], points).unwrap()
        };
        let base = tri((0.0, 0.0), (2.0, 0.0), (0.0, 2.0));
        for (other, overlap, what) in [
            (tri((2.0, 0.0), (2.0, 2.0), (0.0, 2.0)), false, "sharing an edge"),
            (tri((2.0, 0.0), (3.0, 0.0), (3.0, 1.0)), false, "sharing a corner"),
            (tri((3.0, 3.0), (4.0, 3.0), (3.0, 4.0)), false, "apart"),
            (tri((-3.0, -3.0), (-2.0, -3.0), (-1.0, -1.0)), false, "apart, where only the base has an edge between"),
            (tri((0.5, 0.5), (3.0, 0.5), (0.5, 3.0)), true, "across an edge"),
            (tri((0.2, 0.2), (0.6, 0.2), (0.2, 0.6)), true, "inside"),
            (tri((0.0, 0.0), (2.0, 0.0), (0.0, 2.0)), true, "the same"),
            (tri((-1.0, -1.0), (5.0, -1.0), (-1.0, 5.0)), true, "around"),
        ] {
            assert_eq!((base.overlaps(&other), other.overlaps(&base)), (overlap, overlap), "{what}");
        }
    }

    #[test]
    fn the_slack_is_eight_epsilons_of_the_edge_times_the_sizes_in_play() {
        // An edge of length 3 + 4 in the taxicab measure, and corners and a point of sizes 3, 10 and 5.
        let got = slack(at(1.0, 2.0), at(4.0, 6.0), at(2.0, 3.0));
        assert_eq!(got, 8.0 * f64::EPSILON * 7.0 * 18.0);
        assert_eq!(slack(at(-1.0, -2.0), at(-4.0, -6.0), at(-2.0, -3.0)), got, "sizes are magnitudes");
    }

    #[test]
    fn each_region_of_a_triangle_gives_its_own_nearest_point() {
        // Corners at (0, 0, 0), (4, 0, 0) and (0, 4, 0), their places the weights themselves. Beside the far edge, (5, 3)
        // is past the second corner along the first edge and (3, 5) past the third along the second, yet both are
        // nearest on the far edge: only one half of each corner's test holds there.
        let points = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(0.0, 4.0, 0.0)];
        let f = Facet::new([0, 1, 2], [at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)], points).unwrap();
        for (p, uv, gap) in [
            ((-1.0, -1.0, 0.0), (0.0, 0.0), 2.0),
            ((6.0, -1.0, 0.0), (1.0, 0.0), 5.0),
            ((-1.0, 6.0, 0.0), (0.0, 1.0), 5.0),
            ((-1.0, 2.0, 0.0), (0.0, 0.5), 1.0),
            ((3.0, 3.0, 0.0), (0.5, 0.5), 2.0),
            ((5.0, 3.0, 0.0), (0.75, 0.25), 8.0),
            ((3.0, 5.0, 0.0), (0.25, 0.75), 8.0),
            ((1.0, 1.0, 2.0), (0.25, 0.25), 4.0),
        ] {
            let (got, got_gap) = f.nearest(Vec3::new(p.0, p.1, p.2));
            assert!((got.u() - uv.0).abs() < 1e-15 && (got.v() - uv.1).abs() < 1e-15, "{p:?}: {got:?}");
            assert!((got_gap - gap).abs() < 1e-12, "{p:?}: {got_gap}");
        }
    }

    #[test]
    fn a_point_beside_an_edge_and_behind_a_corner_is_nearest_on_the_edge() {
        // Beside the edge from the first corner to the second, and behind the first corner as seen from the third: only
        // the edge's region holds it, not the corner's.
        let points = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(0.0, 4.0, 0.0)];
        let f = Facet::new([0, 1, 2], [at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)], points).unwrap();
        let (uv, gap) = f.nearest(Vec3::new(2.0, -1.0, 0.0));
        assert_eq!((uv, gap), (at(0.5, 0.0), 1.0));
    }

    #[test]
    fn a_triangle_clockwise_or_flat_in_the_parameters_is_refused() {
        let points = [Vec3::new(0.0, 0.0, 0.0); 3];
        assert!(Facet::new([0, 1, 2], [at(0.0, 0.0), at(0.0, 1.0), at(1.0, 0.0)], points).is_none(), "clockwise");
        assert!(Facet::new([0, 1, 2], [at(0.0, 0.0), at(1.0, 1.0), at(2.0, 2.0)], points).is_none(), "flat");
        assert!(Facet::new([0, 1, 2], [at(f64::NAN, 0.0), at(1.0, 0.0), at(0.0, 1.0)], points).is_none());
    }

    #[test]
    fn a_point_on_a_shared_edge_is_in_one_triangle_or_both() {
        // Two triangles on the edge from vertex 3 to vertex 8, which is not axis-aligned, so points on it round.
        let (p, q, left, right) = (at(0.1, 0.2), at(0.7, 0.9), at(0.0, 1.0), at(1.0, 0.0));
        let points = [Vec3::new(0.0, 0.0, 0.0); 3];
        let one = Facet::new([0, 1, 2], [p, q, left], points).unwrap();
        let two = Facet::new([0, 1, 2], [q, p, right], points).unwrap();
        for k in 0..=1000 {
            let f = f64::from(k) / 1000.0;
            let on = at(0.1 + 0.6 * f, 0.2 + 0.7 * f);
            assert!(one.holds(on) || two.holds(on), "{on:?}");
        }
        assert!(one.holds(at(0.2, 0.6)) && !two.holds(at(0.2, 0.6)) && !one.holds(at(f64::NAN, 0.5)));
        assert!(!one.holds(at(0.9, 0.1)) && two.holds(at(0.6, 0.4)));
    }

    #[test]
    fn the_nearest_point_is_in_the_region_the_point_projects_into() {
        let f = facet();
        let (a, b, c) = (f.points[0], f.points[1], f.points[2]);
        let normal = (b - a).cross(c - a);
        let lift = normal * (1.0 / normal.norm());
        let cases = [
            (a - (b - a) - (c - a) + lift, at(0.0, 0.0)),
            (b + (b - a) * 0.5 + lift, at(2.0, 0.0)),
            (c + (c - a) * 0.5 - lift, at(0.0, 1.0)),
            (a + (b - a) * 0.25 - (c - a) * 0.5, at(0.6, 0.0)),
            (a + (c - a) * 0.75 - (b - a) * 0.5 + lift, at(0.0, 0.85)),
            (a + (b - a) * 0.3 + (c - a) * 0.2 + lift * 2.0, at(0.6, 0.2)),
        ];
        for (p, want) in cases {
            let (uv, gap) = f.nearest(p);
            assert!((uv.u() - want.u()).abs() < 1e-12 && (uv.v() - want.v()).abs() < 1e-12, "{p:?}: {uv:?}");
            assert!((gap - (f.jet(uv)[0] - p).dot(f.jet(uv)[0] - p)).abs() < 1e-12);
        }
        // Across from a, past the edge from b to c, in the plane and square to the edge.
        let across = normal.cross(c - b);
        let out = if across.dot(a - b) > 0.0 { across * -1.0 } else { across };
        let (uv, _) = f.nearest(b + (c - b) * 0.4 + out * (0.7 / out.norm()));
        assert!((uv.u() - 1.2).abs() < 1e-12 && (uv.v() - 0.4).abs() < 1e-12, "{uv:?}");
    }

    #[test]
    fn a_triangle_collapsed_in_space_is_nearest_on_an_edge() {
        let line = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
        let f = Facet::new([0, 1, 2], [at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)], line).unwrap();
        let (uv, gap) = f.nearest(Vec3::new(1.5, 1.0, 0.0));
        assert!((gap - 1.0).abs() < 1e-15 && (f.jet(uv)[0] - Vec3::new(1.5, 0.0, 0.0)).norm() < 1e-15, "{uv:?}");
    }
}
