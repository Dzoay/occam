//! Source: shewchuk-1997
//! The two decisions a triangulation in a face's parameters rests on (Shewchuk 1997): which side of a line a point
//! lies on, and whether a point lies inside the circle through three others. P9 made both exact by rounding every point
//! to an integer grid, and two points that the grid made one caused all three of its failures. Here nothing is rounded,
//! so two points are one only where their coordinates are.
//!
//! Which side of a line a point lies on is decided exactly: from its value in plain arithmetic where that lies beyond
//! Shewchuk's first error bound, or where each product has a factor that is a difference of equal coordinates and so
//! is exactly zero; and otherwise by `occam-math`'s filter and exact stage, which one decision costs far more of. Every
//! walk, split and segment rests on this side, so the triangulation is valid whatever the coordinates.
//!
//! Whether a point lies inside a circle decides only which of two triangulations of four points is kept, and both are
//! valid. It is decided from plain arithmetic beyond Shewchuk's first bound alone, and is undecided within it, where
//! the four points are within a rounding of one circle, as the samples of a circle all are; a flip is made only where
//! the point is certainly inside, so every flip is valid and no tie costs the exact stage.

use occam_math::{Predicate, Ring, Side, Verdict, decide_stored};

/// A point of the plane a face is triangulated in.
pub(crate) type P2 = [f64; 2];

/// Twice the signed area of `a`, `b`, `c` from `[ax, ay, bx, by, cx, cy]`: above zero when `c` lies left of `a` to
/// `b`, so that `a`, `b`, `c` run anticlockwise.
struct Orientation;

impl Predicate<6> for Orientation {
    const DEGREE: u32 = 2;

    fn value<T: Ring>(&self, [ax, ay, bx, by, cx, cy]: &[T; 6]) -> T {
        bx.sub(ax).mul(&cy.sub(ay)).sub(&by.sub(ay).mul(&cx.sub(ax)))
    }
}

/// The side `predicate` certainly gives `item` against zero, or `None` where it cannot be decided: a coordinate that
/// is not finite, or beyond the range the exact stage is exact for.
fn decided<const N: usize, P: Predicate<N>>(predicate: &P, item: [f64; N]) -> Option<Side> {
    let mut out = Vec::with_capacity(1);
    decide_stored(predicate, &[item], 0.0, &mut out);
    match out.first() {
        Some(Verdict::Decided(side)) => Some(*side),
        Some(Verdict::Undecided) | None => None,
    }
}

/// Half the distance from 1 to the next number: Shewchuk's epsilon.
const EPSILON: f64 = f64::EPSILON / 2.0;

/// Shewchuk's first error bound for orientation, `(3 + 16e)e`, against the sum of the two products' sizes.
const ORIENT_BOUND: f64 = (3.0 + 16.0 * EPSILON) * EPSILON;

/// Shewchuk's first error bound for the circle, `(10 + 96e)e`, against the permanent.
const INCIRCLE_BOUND: f64 = (10.0 + 96.0 * EPSILON) * EPSILON;

/// How far from the ends of the exponent range what a bound is taken of must be for it to hold: a rounding below the
/// smallest normal number, or an overflow, is not one it allows for.
const SIZES: core::ops::RangeInclusive<f64> = 1e-270..=1e270;

/// The side `value` certainly lies on, if it lies beyond `bound`, and `size`, what the bound was taken of, is in
/// [`SIZES`].
fn beyond(value: f64, bound: f64, size: f64) -> Option<Side> {
    if !SIZES.contains(&size) {
        None
    } else if value > bound {
        Some(Side::Above)
    } else if -value > bound {
        Some(Side::Below)
    } else {
        None
    }
}

/// Which side of the line from `a` to `b` the point `c` lies on: [`Side::Above`] on the left.
pub(crate) fn orient(a: P2, b: P2, c: P2) -> Option<Side> {
    let (ax, ay, bx, by) = (a[0] - c[0], a[1] - c[1], b[0] - c[0], b[1] - c[1]);
    let (left, right) = (ax * by, ay * bx);
    let finite = left.is_finite() && right.is_finite();
    if finite && (ax == 0.0 || by == 0.0) && (ay == 0.0 || bx == 0.0) {
        return Some(Side::Within);
    }
    let size = left.abs() + right.abs();
    beyond(left - right, ORIENT_BOUND * size, size)
        .or_else(|| decided(&Orientation, [a[0], a[1], b[0], b[1], c[0], c[1]]))
}

/// Where `d` lies against the circle through `a`, `b`, `c`, which run anticlockwise: [`Side::Above`] certainly inside
/// it, [`Side::Below`] certainly outside, and `None` within a rounding of it or where a coordinate is not finite.
pub(crate) fn incircle(a: P2, b: P2, c: P2, d: P2) -> Option<Side> {
    let (det, permanent) = lifted(a, b, c, d);
    beyond(det, INCIRCLE_BOUND * permanent, permanent)
}

/// The lifted determinant of `a`, `b`, `c` and `d` in plain arithmetic, and its permanent, which Shewchuk's bound is
/// taken of: each written as he writes them.
#[expect(clippy::similar_names, reason = "the differences and products as Shewchuk names them")]
fn lifted(a: P2, b: P2, c: P2, d: P2) -> (f64, f64) {
    let (adx, ady, bdx, bdy) = (a[0] - d[0], a[1] - d[1], b[0] - d[0], b[1] - d[1]);
    let (cdx, cdy) = (c[0] - d[0], c[1] - d[1]);
    let (bdxcdy, cdxbdy, alift) = (bdx * cdy, cdx * bdy, adx * adx + ady * ady);
    let (cdxady, adxcdy, blift) = (cdx * ady, adx * cdy, bdx * bdx + bdy * bdy);
    let (adxbdy, bdxady, clift) = (adx * bdy, bdx * ady, cdx * cdx + cdy * cdy);
    let det = alift * (bdxcdy - cdxbdy) + blift * (cdxady - adxcdy) + clift * (adxbdy - bdxady);
    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * alift
        + (cdxady.abs() + adxcdy.abs()) * blift
        + (adxbdy.abs() + bdxady.abs()) * clift;
    (det, permanent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_left_of_a_line_is_above_it_and_one_on_it_within() {
        assert_eq!(orient([0.0, 0.0], [1.0, 0.0], [0.5, 1e-100]), Some(Side::Above));
        assert_eq!(orient([0.0, 0.0], [1.0, 0.0], [0.5, -1e-100]), Some(Side::Below));
        // 0.1 and 0.3 are not what they print as, but 0.3 is not three times 0.1 either: only the bits decide.
        assert_eq!(orient([0.0, 0.0], [0.1, 0.1], [0.3, 0.3]), Some(Side::Within));
        let third = 0.1 + 0.2;
        assert_eq!(orient([0.0, 0.0], [1.0, 1.0], [third, 0.3]), Some(Side::Below), "0.1 + 0.2 is above 0.3");
    }

    #[test]
    fn a_point_inside_a_circle_is_above_and_one_within_a_rounding_of_it_undecided() {
        let (a, b, c) = ([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]);
        assert_eq!(incircle(a, b, c, [0.0, 0.0]), Some(Side::Above));
        assert_eq!(incircle(a, b, c, [0.0, -1.0 - 1e-9]), Some(Side::Below));
        assert_eq!(incircle(a, c, b, [0.0, 0.0]), Some(Side::Below), "clockwise turns it over");
        assert_eq!(incircle(a, b, c, [0.0, -1.0]), None, "on it");
        assert_eq!(incircle(a, b, c, [0.0, -1.0 - f64::EPSILON]), None, "a rounding outside it");
    }

    #[test]
    fn the_first_stage_and_the_exact_one_give_the_same_side() {
        // Near a line, where the first stage gives way, far from it, where it decides, and on and beside one of
        // constant `u`, where a difference of equal coordinates decides.
        let (a, b) = ([0.1, 0.2], [0.7, 0.9]);
        for k in -40..=40 {
            let t = f64::from(k) * f64::EPSILON;
            let near = [0.4 + t, 0.55 + t];
            assert_eq!(orient(a, b, near), decided(&Orientation, [a[0], a[1], b[0], b[1], near[0], near[1]]), "{k}");
            let far = [0.4 + 1e6 * t, -0.2];
            assert_eq!(orient(a, b, far), decided(&Orientation, [a[0], a[1], b[0], b[1], far[0], far[1]]), "{k}");
            let on_line = [0.1 + t, 0.4];
            assert_eq!(
                orient([0.1, 0.2], [0.1, 0.9], on_line),
                decided(&Orientation, [0.1, 0.2, 0.1, 0.9, on_line[0], 0.4])
            );
        }
    }

    #[test]
    fn a_coordinate_that_is_not_finite_decides_nothing() {
        assert_eq!(orient([0.0, 0.0], [1.0, 0.0], [f64::NAN, 1.0]), None);
        assert_eq!(incircle([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [f64::INFINITY, 0.0]), None);
        assert_eq!(incircle([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [1e300, 0.0]), None, "beyond the bound's range");
    }
}
