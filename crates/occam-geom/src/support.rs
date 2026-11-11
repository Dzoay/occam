//! Source: original
//! What every family's formulas share: stored numbers in lanes, and boxes that hold the exact values of computed points.

use occam_math::{Interval, Real};
use occam_report::{Code, Fault, NamedValue, Report};
use occam_units::{Bounds, Length, Point, Vec3, World};

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

/// The box holding all of space.
pub(crate) fn everything() -> Bounds {
    Bounds::around(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Length::from_millimetres(f64::INFINITY))
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
    fn a_stored_vector_fills_every_lane() {
        let v: Vec3<occam_math::Lanes<3>> = splat(Vec3::new(1.0, -2.0, 0.5));
        assert_eq!((v.x.to_array(), v.y.to_array(), v.z.to_array()), ([1.0; 3], [-2.0; 3], [0.5; 3]));
    }
}
