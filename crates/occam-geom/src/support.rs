//! Source: original
//! What every family's formulas share: stored numbers in lanes, and boxes that hold the exact values of computed points.

use occam_math::{Interval, Real};
use occam_report::{Code, Fault, NamedValue, Quantity, Reason, Report};
use occam_units::{Bounds, Length, Point, Vec3, World};

use crate::curve::ParametricCurve;
use crate::surface::Closest;

/// A stored vector in every lane.
#[inline(always)]
pub(crate) fn splat<R: Real>(v: Vec3) -> Vec3<R> {
    Vec3::new(R::splat(v.x), R::splat(v.y), R::splat(v.z))
}

/// Exactly `x`, as an interval.
pub(crate) fn exact(x: f64) -> Interval {
    Interval::exact(x)
}

/// The box holding every point whose coordinates lie in these intervals, so it holds the exact values that computed
/// coordinates stand for. A coordinate that is not finite, as an unbounded patch gives, makes it [`everything`].
pub(crate) fn enclose(points: impl IntoIterator<Item = [Interval; 3]>) -> Bounds {
    let corner = |[x, y, z]: [f64; 3]| Point::from_vec3(Vec3::new(x, y, z));
    let corners = points
        .into_iter()
        .flat_map(|[x, y, z]| [corner([x.lower(), y.lower(), z.lower()]), corner([x.upper(), y.upper(), z.upper()])]);
    Bounds::of(corners).unwrap_or_else(everything)
}

/// An interval holding every number from `lo` to `hi`.
pub(crate) fn span(lo: f64, hi: f64) -> Interval {
    let mid = 0.5 * lo + 0.5 * hi;
    let (above, below) = (hi - mid, mid - lo);
    Interval::new(mid, (if below > above { below } else { above }).next_up())
}

/// `v` seen along `along`: with its part along it taken away, or `v` itself where there is none.
pub(crate) fn seen(v: Vec3, along: Option<occam_units::Direction<World>>) -> Vec3 {
    along.map_or(v, |d| {
        let d = d.get();
        v - d * v.dot(d)
    })
}

/// The box holding all of space.
pub(crate) fn everything() -> Bounds {
    Bounds::around(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Length::from_millimetres(f64::INFINITY))
}

/// `hull` padded for rounding: 64 machine epsilons of its largest coordinate for each of `terms` sums that computed a
/// point it is meant to hold.
pub(crate) fn padded(hull: Bounds, terms: f64) -> Bounds {
    let (low, high) = (hull.low().to_vec3(), hull.high().to_vec3());
    let size = [low.x, low.y, low.z, high.x, high.y, high.z]
        .into_iter()
        .fold(0.0, |a, x| if x.abs() > a { x.abs() } else { a });
    let margin = Length::from_millimetres(64.0 * terms * f64::EPSILON * size);
    Bounds::around(hull.low(), margin).union(&Bounds::around(hull.high(), margin))
}

/// What a curve's [`Closest`] is when its foot is at `t`.
pub(crate) fn foot_at<C: ParametricCurve>(curve: &C, p: Point<World>, t: f64) -> Closest<f64> {
    let point = curve.eval_point(t);
    Closest { at: t, point, distance: Length::from_millimetres((p.to_vec3() - point.to_vec3()).norm()) }
}

/// `size` in millimetres, which must be a finite number greater than zero, as a radius is.
pub(crate) fn positive(size: Length) -> Result<f64, Report> {
    let millimetres = size.millimetres();
    if millimetres > 0.0 && millimetres.is_finite() { Ok(millimetres) } else { Err(not_positive(millimetres)) }
}

/// `size` in millimetres, which must be a finite number of either sign, as an offset's distance is.
pub(crate) fn finite_size(size: Length) -> Result<f64, Report> {
    let millimetres = size.millimetres();
    if millimetres.is_finite() {
        Ok(millimetres)
    } else {
        Err(Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::length("value", millimetres)))
    }
}

/// `size` in millimetres, which must be a finite number not below zero, as a cone's radius where it is placed is. Zero
/// of either sign is +0, so that equal sizes have equal bits.
pub(crate) fn not_negative(size: Length) -> Result<f64, Report> {
    let millimetres = size.millimetres();
    if millimetres >= 0.0 && millimetres.is_finite() { Ok(millimetres.abs()) } else { Err(not_positive(millimetres)) }
}

/// The refusal of a size of `millimetres`.
fn not_positive(millimetres: f64) -> Report {
    Report::new(Code::SizeNotPositive, Fault::InvalidInput)
        .with_reason(Reason::RadiusNotPositive)
        .with_value(NamedValue::length("size", millimetres))
}

/// `value`, which must lie strictly between `low` and `high`, all measuring `quantity`.
pub(crate) fn between(value: f64, (low, high): (f64, f64), quantity: Quantity) -> Result<f64, Report> {
    if low < value && value < high {
        return Ok(value);
    }
    Err(Report::new(Code::OutOfRange, Fault::InvalidInput)
        .with_value(NamedValue::new("value", quantity, value))
        .with_value(NamedValue::new("low", quantity, low))
        .with_value(NamedValue::new("high", quantity, high)))
}

/// Refuses a point with a coordinate that is not finite, naming the first.
pub(crate) fn finite(p: Point<World>) -> Result<(), Report> {
    let v = p.to_vec3();
    match [v.x, v.y, v.z].into_iter().find(|x| !x.is_finite()) {
        None => Ok(()),
        Some(x) => Err(Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::length("value", x))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64, z: f64) -> Point<occam_units::World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    #[test]
    fn the_box_holds_every_interval() {
        let (a, b) = (exact(0.1) + exact(0.2), exact(-3.0) * exact(1.0 / 3.0));
        let b_box = enclose([[a, b, exact(2.0)], [b, a, exact(-2.0)]]);
        assert!(b_box.low().x().millimetres() <= a.lower() && b_box.low().x().millimetres() <= b.lower());
        assert_eq!(b_box.low().x().millimetres(), b.lower());
        assert_eq!(b_box.high().y().millimetres(), a.upper());
        let z = (b_box.low().z().millimetres(), b_box.high().z().millimetres());
        assert_eq!(z, (exact(-2.0).lower(), exact(2.0).upper()));
        assert!(z.0 < -2.0 && z.1 > 2.0, "even an exact interval is stepped outward");
        assert!(a.lower() < a.upper(), "the sum rounded, so its interval has width");
    }

    #[test]
    fn a_coordinate_that_is_not_finite_gives_all_of_space() {
        let all = enclose([[exact(1.0), exact(f64::INFINITY), exact(0.0)]]);
        assert!(all.contains(point(1e308, -1e308, 0.0)) && all.contains(point(-1e308, 1e308, 1e308)));
        assert!(everything().contains(point(f64::MAX, f64::MIN, 0.0)));
        assert!(enclose([[exact(f64::NAN), exact(0.0), exact(0.0)]]).contains(point(-5.0, 7.0, 1e300)));
    }

    #[test]
    fn a_point_that_is_not_finite_is_refused_by_its_first_bad_coordinate() {
        assert!(finite(point(1e308, -1e308, 0.0)).is_ok());
        for (p, bad) in [(point(f64::NAN, 1.0, 2.0), f64::NAN), (point(0.0, f64::INFINITY, f64::NAN), f64::INFINITY)] {
            let report = finite(p).unwrap_err();
            assert_eq!((report.code, report.fault), (Code::NotFinite, Some(Fault::InvalidInput)));
            assert_eq!(report.value("value").and_then(NamedValue::number).map(f64::to_bits), Some(bad.to_bits()));
        }
        assert_eq!(finite(point(0.0, 0.0, f64::NEG_INFINITY)).unwrap_err().code, Code::NotFinite);
    }

    #[test]
    fn a_span_holds_both_ends() {
        for (lo, hi) in [(0.1, 0.7), (-3.0, 1e-300), (2.5, 2.5), (-1e300, 1e300), (1.0 / 3.0, 2.0 / 3.0)] {
            let held = span(lo, hi);
            assert!(held.lower() <= lo && held.upper() >= hi, "{lo} {hi}: {held:?}");
            assert!(held.upper() - held.lower() <= (hi - lo) * 1.000_001 + 1e-15 * (lo.abs() + hi.abs()), "{lo} {hi}");
        }
        let lopsided = span(1.0, 1.0 + 2.0 * f64::EPSILON);
        assert!(lopsided.lower() <= 1.0 && lopsided.upper() >= 1.0 + 2.0 * f64::EPSILON);
        // The midpoint of 2^-10 + 2^-53 and 1 + 2^-52 rounds up by 2^-54, so the half below it is the larger by 2^-53,
        // more than the step down from the low end, which is much finer than the midpoint's.
        let (lo, hi) = (1.0 / 1024.0 + f64::EPSILON / 2.0, 1.0 + f64::EPSILON);
        let uneven = span(lo, hi);
        assert!(uneven.lower() <= lo && uneven.upper() >= hi, "{uneven:?}");
    }

    #[test]
    fn a_size_must_be_finite_and_above_zero() {
        assert_eq!(positive(Length::from_millimetres(2.5)).unwrap(), 2.5);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let report = positive(Length::from_millimetres(bad)).unwrap_err();
            assert_eq!((report.code, report.reason), (Code::SizeNotPositive, Some(Reason::RadiusNotPositive)));
            let size = report.value("size").and_then(NamedValue::number).unwrap();
            assert_eq!(size.to_bits(), bad.to_bits());
        }
    }

    #[test]
    fn a_size_that_may_be_zero_must_be_finite_and_not_below_it() {
        assert_eq!(not_negative(Length::from_millimetres(2.5)).unwrap(), 2.5);
        assert_eq!(not_negative(Length::from_millimetres(-0.0)).unwrap().to_bits(), 0.0f64.to_bits(), "+0, not -0");
        for bad in [-1e-300, f64::NAN, f64::INFINITY] {
            let report = not_negative(Length::from_millimetres(bad)).unwrap_err();
            assert_eq!((report.code, report.reason), (Code::SizeNotPositive, Some(Reason::RadiusNotPositive)));
        }
    }

    #[test]
    fn a_value_must_lie_strictly_between_its_bounds() {
        assert_eq!(between(0.5, (0.0, 1.0), Quantity::Angle).unwrap(), 0.5);
        for bad in [0.0, 1.0, -0.5, 2.0, f64::NAN] {
            let report = between(bad, (0.0, 1.0), Quantity::Angle).unwrap_err();
            assert_eq!((report.code, report.fault), (Code::OutOfRange, Some(Fault::InvalidInput)));
            let named = |name: &str| report.value(name).map(|v| (v.number().map(f64::to_bits), v.quantity));
            assert_eq!(named("value"), Some((Some(bad.to_bits()), Quantity::Angle)));
            assert_eq!(
                (named("low"), named("high")),
                (Some((Some(0), Quantity::Angle)), Some((Some(1f64.to_bits()), Quantity::Angle)))
            );
        }
    }

    #[test]
    fn a_padded_box_grows_by_epsilons_of_its_largest_coordinate() {
        // Its lowest x and z are 0, where the margin can be read to a part in a thousand; its largest coordinate is 4.
        let grown = padded(Bounds::of([point(0.0, 1.0, 2.0), point(4.0, 3.0, 0.0)]).unwrap(), 2.0);
        let margin = 128.0 * f64::EPSILON * 4.0;
        for got in [-grown.low().x().millimetres(), -grown.low().z().millimetres()] {
            assert!((got - margin).abs() <= 1e-3 * margin, "{got} against {margin}");
        }
    }

    #[test]
    fn a_stored_vector_fills_every_lane() {
        let v: Vec3<occam_math::Lanes<3>> = splat(Vec3::new(1.0, -2.0, 0.5));
        assert_eq!((v.x.to_array(), v.y.to_array(), v.z.to_array()), ([1.0; 3], [-2.0; 3], [0.5; 3]));
    }
}
