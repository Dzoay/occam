//! Source: hormann-agathos-2001
//! The winding number of a closed chain of segments about a point. Each segment that crosses the point's line of
//! constant `v` upward, with the point on its left, counts one; each that crosses it downward, with the point on its
//! right, counts minus one; a segment's lower end counts as on the line and its upper end does not, so a crossing at a
//! shared end is counted once. Which side the point is on is decided exactly on the numbers given (`decide_stored`,
//! Shewchuk 1997), so the count is the same on every target, and a point on a segment gives no count at all.

use occam_math::{Predicate, Ring, Side, Verdict, decide_stored};
use occam_units::UvPoint;

/// Twice the signed area of the triangle `a`, `b`, `q`, from `[au, av, bu, bv, qu, qv]`: above zero when `q` is on the
/// left of `a` to `b`.
struct Orientation;

impl Predicate<6> for Orientation {
    const DEGREE: u32 = 2;

    fn value<T: Ring>(&self, inputs: &[T; 6]) -> T {
        let [au, av, bu, bv, qu, qv] = inputs;
        bu.sub(au).mul(&qv.sub(av)).sub(&bv.sub(av).mul(&qu.sub(au)))
    }
}

/// The winding number of the closed chain `segments` about `q`, or `None` where `q` lies on a segment that crosses
/// its line, or a side cannot be decided exactly.
pub(super) fn number(segments: &[[UvPoint; 2]], q: UvPoint) -> Option<i32> {
    let (mut crossing, mut upward) = (Vec::new(), Vec::new());
    for [a, b] in segments {
        let up = a.v() <= q.v() && q.v() < b.v();
        let down = b.v() <= q.v() && q.v() < a.v();
        if up || down {
            crossing.push([a.u(), a.v(), b.u(), b.v(), q.u(), q.v()]);
            upward.push(up);
        }
    }
    let mut verdicts = Vec::new();
    decide_stored(&Orientation, &crossing, 0.0, &mut verdicts);
    let mut winding = 0_i32;
    for (verdict, up) in verdicts.into_iter().zip(upward) {
        winding = match (verdict, up) {
            (Verdict::Decided(Side::Above), true) => winding.checked_add(1)?,
            (Verdict::Decided(Side::Below), false) => winding.checked_sub(1)?,
            (Verdict::Decided(Side::Above | Side::Below), _) => winding,
            (Verdict::Decided(Side::Within) | Verdict::Undecided, _) => return None,
        };
    }
    Some(winding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uv(u: f64, v: f64) -> UvPoint {
        UvPoint::new(u, v)
    }

    /// The closed chain through `points`, back to the first.
    #[expect(clippy::arithmetic_side_effects, reason = "small indices in a test")]
    fn chain(points: &[(f64, f64)]) -> Vec<[UvPoint; 2]> {
        let at = |k: usize| uv(points[k % points.len()].0, points[k % points.len()].1);
        (0..points.len()).map(|k| [at(k), at(k + 1)]).collect()
    }

    #[test]
    fn a_point_inside_an_anticlockwise_square_winds_once_and_a_clockwise_one_minus_once() {
        let square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_eq!(number(&chain(&square), uv(0.5, 0.5)), Some(1));
        let reversed: Vec<(f64, f64)> = square.iter().rev().copied().collect();
        assert_eq!(number(&chain(&reversed), uv(0.5, 0.5)), Some(-1));
        for outside in [uv(2.0, 0.5), uv(-1.0, 0.5), uv(0.5, 2.0), uv(0.5, -1.0), uv(-1.0, 0.0), uv(2.0, 1.0)] {
            assert_eq!(number(&chain(&square), outside), Some(0), "{outside:?}");
        }
    }

    #[test]
    fn a_line_through_a_corner_is_counted_once() {
        let diamond = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
        assert_eq!(number(&chain(&diamond), uv(0.0, 0.0)), Some(1), "level with two corners");
        assert_eq!(number(&chain(&diamond), uv(-2.0, 0.0)), Some(0), "left of the corner it is level with");
        assert_eq!(number(&chain(&diamond), uv(2.0, 0.0)), Some(0), "right of it");
    }

    #[test]
    fn a_point_on_a_crossing_segment_has_no_number() {
        let square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_eq!(number(&chain(&square), uv(1.0, 0.5)), None);
        assert_eq!(number(&chain(&square), uv(0.0, 0.25)), None);
        assert_eq!(number(&chain(&square), uv(f64::NAN, 0.5)), None, "a side that cannot be decided");
    }

    #[test]
    fn a_chain_that_goes_round_twice_winds_twice() {
        let twice =
            [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
        assert_eq!(number(&chain(&twice), uv(0.1, 0.1)), Some(2));
        assert_eq!(number(&[], uv(0.0, 0.0)), Some(0));
    }

    #[test]
    fn the_orientation_is_twice_the_signed_area() {
        let value = Orientation.value(&[0.0, 0.0, 2.0, 0.0, 0.0, 3.0]);
        assert_eq!(value, 6.0);
        assert_eq!(Orientation.value(&[1.0, 1.0, 3.0, 1.0, 1.0, -2.0]), -6.0);
    }
}
