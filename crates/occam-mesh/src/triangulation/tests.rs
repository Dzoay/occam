//! Source: original
#![expect(clippy::arithmetic_side_effects, reason = "small indices and sums in tests")]

use occam_math::Side;

use super::*;
use crate::predicates::{P2, incircle};

/// The closed chain round `from..to`, in order.
fn ring(from: usize, to: usize) -> Vec<[usize; 2]> {
    (from..to).map(|k| [k, if k + 1 == to { from } else { k + 1 }]).collect()
}

/// Twice the signed area of the triangle at `corners` of `points`.
fn doubled(points: &[P2], [a, b, c]: [usize; 3]) -> f64 {
    let (p, q, r) = (points[a], points[b], points[c]);
    (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
}

/// The area the inside triangles of `mesh` cover, each of which runs anticlockwise.
fn area(mesh: &Triangulation, points: &[P2]) -> f64 {
    let each = |t: [usize; 3]| {
        let twice = doubled(points, t);
        assert!(twice > 0.0, "{t:?} runs clockwise or has no area");
        twice / 2.0
    };
    mesh.triangles().into_iter().map(each).sum()
}

/// The corners of the square from `(lo, lo)` to `(hi, hi)`, anticlockwise.
fn square(lo: f64, hi: f64) -> Vec<P2> {
    vec![[lo, lo], [hi, lo], [hi, hi], [lo, hi]]
}

/// The middle of the triangle at `corners` of `points`.
fn middle(points: &[P2], [a, b, c]: [usize; 3]) -> P2 {
    let (p, q, r) = (points[a], points[b], points[c]);
    [(p[0] + q[0] + r[0]) / 3.0, (p[1] + q[1] + r[1]) / 3.0]
}

/// A 3 by 3 square with a hole of 1 by 1 in its middle, running clockwise, the whole turned over if `over`.
fn holed(over: bool) -> (Vec<P2>, Vec<[usize; 2]>) {
    let mut points = square(0.0, 3.0);
    points.extend(square(1.0, 2.0).into_iter().rev());
    if over {
        points[..4].reverse();
        points[4..].reverse();
    }
    let mut segments = ring(0, 4);
    segments.extend(ring(4, 8));
    (points, segments)
}

#[test]
fn a_square_is_two_triangles_covering_it() {
    let points = square(0.0, 1.0);
    let mesh = Triangulation::new(&points, &ring(0, 4)).unwrap();
    assert_eq!((mesh.triangles().len(), mesh.count()), (2, 2));
    assert_eq!(area(&mesh, &points), 1.0);
}

#[test]
fn a_square_with_a_hole_leaves_the_hole_empty_whichever_way_its_loops_run() {
    for over in [false, true] {
        let (points, segments) = holed(over);
        let mesh = Triangulation::new(&points, &segments).unwrap();
        assert_eq!(mesh.triangles().len(), 8, "turned over: {over}");
        assert_eq!(area(&mesh, &points), 8.0);
        for t in mesh.triangles() {
            let [u, v] = middle(&points, t);
            assert!(!(1.0 < u && u < 2.0 && 1.0 < v && v < 2.0), "{t:?} is in the hole");
        }
    }
}

#[test]
fn points_along_a_boundary_split_its_segments_and_every_triangle_has_area() {
    // A rectangle 4 by 1 with a point at each unit along its long sides: five collinear points on each.
    let mut points: Vec<P2> = (0..=4).map(|k| [f64::from(k), 0.0]).collect();
    points.extend((0..=4).rev().map(|k| [f64::from(k), 1.0]));
    let mesh = Triangulation::new(&points, &ring(0, 10)).unwrap();
    assert_eq!(mesh.triangles().len(), 8, "a polygon of ten corners and no point inside");
    assert_eq!(area(&mesh, &points), 4.0);
}

/// Whether the inside triangles of `mesh` have an edge from `a` to `b`, either way.
fn has_edge(mesh: &Triangulation, [a, b]: [usize; 2]) -> bool {
    let sides = |[p, q, r]: [usize; 3]| [[p, q], [q, r], [r, p]];
    mesh.triangles().into_iter().flat_map(sides).any(|e| e == [a, b] || e == [b, a])
}

#[test]
fn a_boundary_across_many_delaunay_edges_is_recovered() {
    // A 6 by 6 grid on the unit square bounded by its outer points, and a thin hole along `y = x + 0.1`, between the
    // grid's diagonals, so that the hole's long sides cross the edges the grid's Delaunay triangulation has there.
    let at = |i: i32, j: i32| [f64::from(i) / 5.0, f64::from(j) / 5.0];
    let mut points: Vec<P2> = (0..5).map(|i| at(i, 0)).collect();
    points.extend((0..5).map(|j| at(5, j)));
    points.extend((1..=5).rev().map(|i| at(i, 5)));
    points.extend((1..=5).rev().map(|j| at(0, j)));
    let hole = [[0.05, 0.14], [0.86, 0.95], [0.84, 0.95], [0.05, 0.16]];
    points.extend(hole);
    points.extend((1..5).flat_map(|i| (1..5).map(move |j| at(i, j))));
    let mut segments = ring(0, 20);
    segments.extend(ring(20, 24));
    let mesh = Triangulation::new(&points, &segments).unwrap();
    for &segment in &segments {
        assert!(has_edge(&mesh, segment), "{segment:?} is not an edge");
    }
    let hole_area = f64::midpoint(doubled(&points, [20, 21, 22]), doubled(&points, [20, 22, 23]));
    assert!((area(&mesh, &points) - (1.0 - hole_area)).abs() < 1e-14, "the hole is {hole_area}");
    for t in mesh.triangles() {
        let [u, v] = middle(&points, t);
        assert!(!(0.05 < u && u < 0.86 && 0.09 < v - u && v - u < 0.11), "{t:?} is in the hole");
    }
}

/// A pseudo-random number from 0 to 1, from a linear congruential generator's top bits.
fn random(state: &mut u64) -> f64 {
    *state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    f64::from(u32::try_from(*state >> 32).unwrap()) / f64::from(u32::MAX)
}

#[test]
fn no_inside_triangle_has_the_corner_across_a_free_edge_inside_its_circle() {
    let mut state = 7;
    let mut points = square(0.0, 1.0);
    points.extend((0..300).map(|_| [random(&mut state), random(&mut state)]));
    let mesh = Triangulation::new(&points, &ring(0, 4)).unwrap();
    assert_eq!(mesh.triangles().len(), 2 * 300 + 2, "every point inside");
    for (t, tri) in mesh.tris.iter().enumerate().filter(|&(t, _)| mesh.inside[t]) {
        for k in 0..3 {
            let u = tri.n[k];
            if tri.fixed[k] || u == NONE || !mesh.inside[u] {
                continue;
            }
            let far = mesh.tris[u].v.into_iter().find(|v| !tri.v.contains(v)).unwrap();
            let [a, b, c] = tri.v.map(|v| mesh.points[v]);
            assert_ne!(incircle(a, b, c, mesh.points[far]), Some(Side::Above), "triangle {t}, edge {k}");
        }
    }
}

#[test]
fn two_points_at_one_place_are_refused() {
    let mut points = square(0.0, 1.0);
    points.push([1.0, 0.0]);
    assert_eq!(Triangulation::new(&points, &ring(0, 4)).err(), Some(Fail::Coincide(1, 4)));
}

#[test]
fn a_boundary_that_crosses_itself_is_refused() {
    let points = vec![[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]];
    let failed = Triangulation::new(&points, &ring(0, 4)).err();
    assert!(matches!(failed, Some(Fail::Crosses(..))), "{failed:?}");
}

#[test]
fn a_boundary_through_a_point_of_another_is_refused() {
    // A triangle whose base runs through a corner of its hole.
    let points = vec![[0.0, 0.0], [4.0, 0.0], [2.0, 4.0], [2.0, 0.0], [1.5, 1.0], [2.5, 1.0]];
    let mut segments = ring(0, 3);
    segments.extend(ring(3, 6));
    assert_eq!(Triangulation::new(&points, &segments).err(), Some(Fail::Touches([0, 1], 3)));
}

#[test]
fn a_loop_of_two_points_bounds_nothing_and_is_refused() {
    let points = vec![[0.0, 0.0], [1.0, 0.0]];
    let failed = Triangulation::new(&points, &[[0, 1], [1, 0]]).err();
    assert_eq!(failed, Some(Fail::Crosses([1, 0], [0, 1])));
}

#[test]
fn a_point_placed_in_a_triangle_splits_it_and_one_on_a_fixed_edge_or_outside_is_not_placed() {
    let mut points = square(0.0, 1.0);
    let mut mesh = Triangulation::new(&points, &ring(0, 4)).unwrap();
    let first = mesh.inside()[0];
    let corners = mesh.corners(first).unwrap();
    let inner = middle(&points, corners);
    assert_eq!(mesh.place_in(first, inner), Ok(Some(4)));
    points.push(inner);
    assert_eq!(mesh.count(), 4);
    assert_eq!(area(&mesh, &points), 1.0);
    assert_ne!(mesh.take_touched().len(), 0);
    assert!(mesh.take_touched().is_empty(), "taken once");
    let (on_side, beyond) = ([0.5, 0.0], [2.0, 2.0]);
    for t in mesh.inside() {
        assert_eq!(mesh.place_in(t, on_side), Ok(None), "on a fixed edge");
        assert_eq!(mesh.place_in(t, beyond), Ok(None), "outside it");
    }
    assert_eq!((mesh.count(), mesh.triangles().len()), (4, 4));
}

#[test]
fn coordinates_that_are_not_finite_decide_nothing() {
    let mut points = square(0.0, 1.0);
    points.push([f64::NAN, 0.5]);
    assert_eq!(Triangulation::new(&points, &ring(0, 4)).err(), Some(Fail::Undecided));
    assert_eq!(Triangulation::new(&[[0.0, 0.0]; 3], &[]).err(), Some(Fail::Coincide(0, 1)));
}
