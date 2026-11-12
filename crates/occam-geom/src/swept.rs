//! Source: original
//! What the surfaces swept from a curve share (PARITY 2.2): where along the curve a foot is searched for, which is
//! finite though the curve need not be (`docs/decisions/2026-11-12-unbounded-sweeps.md`); the curve's family and
//! numbers inside the surface's content; and its shape's values inside the surface's.

use occam_report::NamedValue;
use occam_units::{Bounds, Direction, Point, World};

use crate::curve::{ParametricCurve, holding};
use crate::domain::UvBox;
use crate::identity::Content;

/// How much wider than the bound it was given a window is searched, so that rounding leaves out no parameter at it.
const WIDER: f64 = 1.0 / 1_048_576.0;

/// Where along `curve` a swept surface's foot can be: the range, where it is finite, and otherwise the parameters
/// within the radius `near` gives of its centre, seen along its direction where it has one, and its anchor, a parameter
/// at which the surface comes within that radius. `near` is asked only for an unbounded curve.
pub(crate) fn window<C: ParametricCurve>(
    curve: &C,
    near: impl FnOnce() -> (f64, Point<World>, Option<Direction<World>>, f64),
) -> (f64, f64) {
    let range = curve.range();
    if range.lo.is_finite() && range.hi.is_finite() {
        return (range.lo, range.hi);
    }
    let (anchor, centre, along, radius) = near();
    holding(curve.within(centre, along, radius * (1.0 + WIDER)), anchor)
}

/// Adds what defines `curve` to a swept surface's content: its family's name, then its numbers.
pub(crate) fn push_curve<C: ParametricCurve>(out: &mut Content, curve: &C) {
    out.push_family(curve.family());
    curve.content(out);
}

/// `curve`'s shape's values, each named with `curve-` before its own name, after the surface's own `values`.
pub(crate) fn with_curve<C: ParametricCurve>(mut values: Vec<NamedValue>, curve: &C) -> Vec<NamedValue> {
    values.extend(curve.shape().values.into_iter().map(|v| NamedValue { name: format!("curve-{}", v.name), ..v }));
    values
}

/// The lower and the higher of two numbers.
pub(crate) fn ordered(a: f64, b: f64) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

/// Whether every end of `patch` is finite.
pub(crate) fn finite(patch: UvBox) -> bool {
    [patch.lo.u(), patch.lo.v(), patch.hi.u(), patch.hi.v()].into_iter().all(f64::is_finite)
}

/// The eight corners of `b`, as coordinates.
pub(crate) fn corners(b: &Bounds) -> [occam_units::Vec3; 8] {
    let (lo, hi) = (b.low().to_vec3(), b.high().to_vec3());
    let pick = |k: u8| {
        let take = |bit: u8, low: f64, high: f64| if k & bit == 0 { low } else { high };
        occam_units::Vec3::new(take(1, lo.x, hi.x), take(2, lo.y, hi.y), take(4, lo.z, hi.z))
    };
    [pick(0), pick(1), pick(2), pick(3), pick(4), pick(5), pick(6), pick(7)]
}

#[cfg(test)]
mod tests {
    use occam_units::{UvPoint, Vec3};

    use super::*;
    use crate::{FamilyName, Line};

    fn line() -> Line {
        Line::new(Point::<World>::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(0.0, 0.0, 1.0)))
    }

    #[test]
    fn a_window_is_the_range_where_it_is_finite_and_else_near_the_anchor() {
        let segment = line().trimmed(-1.0, 2.0).unwrap();
        let unasked =
            || -> (f64, Point<World>, Option<Direction<World>>, f64) { panic!("a finite range needs no bound") };
        assert_eq!(window(&segment, unasked), (-1.0, 2.0));
        // The line runs along Z through (1, 2, 3): the point (5, 2, 10) is 4 from it, at the parameter 7.
        let centre = Point::from_vec3(Vec3::new(5.0, 2.0, 10.0));
        let (lo, hi) = window(&line(), || (9.0, centre, None, 5.0));
        assert!(lo <= 2.0 && lo > 2.0 - 1e-5 && (hi - 12.0).abs() < 1e-5, "{lo} {hi}");
        assert_eq!(window(&line(), || (-30.0, centre, None, 5.0)).0, -30.0, "the anchor is held");
        let along = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(window(&line(), || (4.0, centre, Some(along), 5.0)), (4.0, 4.0), "no window along the line");
    }

    #[test]
    fn a_held_curve_adds_its_name_numbers_and_values() {
        let segment = line().trimmed(0.0, 1.0).unwrap();
        let (mut out, mut alone) = (Content::default(), Content::default());
        out.start(FamilyName("swept"));
        push_curve(&mut out, &segment);
        alone.push_family(FamilyName("line"));
        segment.content(&mut alone);
        assert!(out.words().ends_with(alone.words()) && out.words().len() == 6 + alone.words().len());
        let values = with_curve(vec![NamedValue::length("a", 1.0)], &segment);
        assert_eq!((values[0].name.as_str(), values[1].name.as_str(), values.len()), ("a", "curve-origin", 5));
    }

    #[test]
    fn a_box_has_eight_corners_and_a_patch_is_finite_or_not() {
        let b = Bounds::of([
            Point::<World>::from_vec3(Vec3::new(0.0, 0.0, 0.0)),
            Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)),
        ])
        .unwrap();
        let all = corners(&b);
        assert!(all.iter().all(|c| b.contains(Point::from_vec3(*c))));
        assert_eq!((all[0], all[7]), (b.low().to_vec3(), b.high().to_vec3()));
        assert_eq!(all[5], Vec3::new(b.high().to_vec3().x, b.low().to_vec3().y, b.high().to_vec3().z));
        assert!(finite(UvBox::new(UvPoint::new(0.0, 1.0), UvPoint::new(2.0, 3.0))));
        assert!(!finite(UvBox::new(UvPoint::new(0.0, 1.0), UvPoint::new(2.0, f64::INFINITY))));
        assert_eq!((ordered(2.0, 1.0), ordered(1.0, 2.0)), ((1.0, 2.0), (1.0, 2.0)));
    }
}
