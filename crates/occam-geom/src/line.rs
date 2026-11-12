//! Source: original
//! The line: `origin + t direction`, where `direction` is a unit axis, so the parameter is a length.

use occam_math::{Interval, Real};
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Direction, ParamKind, Point, Tangent, Tolerance, Vec3, World};

use crate::axes::direction;
use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::exact;
use crate::identity::{Content, FamilyName, Shape};
use crate::support::{enclose, exact, foot_at, splat};
use crate::surface::Closest;

/// A line, unbounded or trimmed to a segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    origin: Vec3,
    direction: Vec3,
    range: ParamRange,
}

impl Line {
    /// The line through `origin` along `direction`, unbounded, with the parameter the distance from `origin`.
    #[must_use]
    pub fn new(origin: Point<World>, direction: Direction<World>) -> Self {
        Self { origin: origin.to_vec3(), direction: direction.get(), range: ParamRange::unbounded(ParamKind::Length) }
    }

    /// The segment from parameter `lo` to `hi`.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`].
    pub fn trimmed(self, lo: f64, hi: f64) -> Result<Self, Report> {
        Ok(Self { range: self.range.trimmed(lo, hi)?, ..self })
    }

    /// Intervals holding the exact coordinates of the point at `t`.
    fn reach(&self, t: f64) -> [Interval; 3] {
        let t = exact(t);
        let (o, d) = (self.origin, self.direction);
        [exact(o.x) + exact(d.x) * t, exact(o.y) + exact(d.y) * t, exact(o.z) + exact(d.z) * t]
    }
}

impl ParametricCurve for Line {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        Point::from_vec3(splat(self.origin) + splat(self.direction) * t)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        (self.eval_point(t), Tangent::new(splat(self.direction)))
    }

    fn derivatives(
        &self,
        _t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        for k in 1..=order {
            out.push(Tangent::new(if k == 1 { self.direction } else { Vec3::new(0.0, 0.0, 0.0) }));
        }
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        enclose([self.reach(lo), self.reach(hi)])
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<f64>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<f64, Report> {
        Ok(self.range.clamp((p.to_vec3() - self.origin).dot(self.direction)))
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        Ok(foot_at(self, p, ParametricCurve::invert(self, p, seed, tol, scratch)?))
    }

    fn family(&self) -> FamilyName {
        FamilyName("line")
    }

    fn content(&self, out: &mut Content) {
        out.push_vec3(self.origin);
        out.push_vec3(self.direction);
        out.push(self.range.lo);
        out.push(self.range.hi);
    }

    fn shape(&self) -> Shape {
        let (o, d) = (self.origin, self.direction);
        let direction = direction("direction", d);
        let (from, to) = (NamedValue::length("from", self.range.lo), NamedValue::length("to", self.range.hi));
        Shape { form: "line", values: vec![NamedValue::point("origin", [o.x, o.y, o.z]), direction, from, to] }
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        let part = self.range.trimmed(lo, hi).ok()?;
        let at = |t: f64| self.origin + self.direction * t;
        Some(exact::curve(1, exact::span(1, (part.lo, part.hi), vec![at(part.lo), at(part.hi)]), false))
    }
}

#[cfg(test)]
mod tests {
    use occam_report::{Code, Quantity, Value};

    use super::*;
    use crate::conformance::{self, Check, WrongSecondCurve, tolerance};
    use crate::curve::Curve;

    fn line() -> Line {
        let along = Direction::normalised(Vec3::new(2.0, -1.0, 0.5));
        Line::new(Point::from_vec3(Vec3::new(12.0, 3.5, -7.0)), along)
    }

    #[test]
    fn the_line_passes_the_conformance_suite() {
        let failures = conformance::curve(&line(), -50.0, 70.0);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn a_segment_passes_the_conformance_suite() {
        let failures = conformance::curve(&line().trimmed(-20.0, 30.0).unwrap(), -20.0, 30.0);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn a_point_beyond_a_segment_is_closest_to_its_end() {
        let segment = line().trimmed(-20.0, 30.0).unwrap();
        let beyond = Point::from_vec3(segment.point(30.0).to_vec3() + segment.first(0.0).1.rate() * 4.0);
        let c = Curve::closest(&segment, beyond, None, tolerance(), &mut Scratch::default()).unwrap();
        assert_eq!((c.at, c.point), (30.0, segment.point(30.0)));
        assert!((c.distance.millimetres() - 4.0).abs() < 1e-12);
        let before = Point::from_vec3(segment.point(-20.0).to_vec3() - segment.first(0.0).1.rate() * 2.0);
        assert_eq!(Curve::invert(&segment, before, None, tolerance(), &mut Scratch::default()).unwrap(), -20.0);
        assert_eq!(Curve::range(&segment).lo, -20.0);
        let mut content = Content::default();
        Curve::content(&segment, &mut content);
        assert_eq!(content.words()[11..], [(-20.0f64).to_bits(), 30.0f64.to_bits()]);
        assert!(line().trimmed(5.0, 5.0).is_err());
    }

    #[test]
    fn the_suite_catches_a_line_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&WrongSecondCurve(line()), -50.0, 70.0));
        assert_eq!(failed, [Check::Differences]);
    }

    #[test]
    fn the_parameter_is_the_distance_along_the_direction() {
        let c = line();
        let d = Direction::<World>::normalised(Vec3::new(2.0, -1.0, 0.5)).get();
        let at = c.point(3.0).to_vec3();
        assert!(((at - Vec3::new(12.0, 3.5, -7.0)) - d * 3.0).norm() < 1e-14);
        assert_eq!(c.first(3.0), (c.point(3.0), Tangent::new(d)));
        let mut out = vec![Tangent::new(Vec3::new(9.0, 9.0, 9.0))];
        Curve::derivatives(&c, 3.0, 3, &mut out, &mut Scratch::default()).unwrap();
        assert_eq!(
            out,
            [Tangent::new(d), Tangent::new(Vec3::new(0.0, 0.0, 0.0)), Tangent::new(Vec3::new(0.0, 0.0, 0.0))]
        );
        let refused = Curve::derivatives(&c, f64::NAN, 1, &mut out, &mut Scratch::default());
        assert_eq!(refused.unwrap_err().code, Code::UndefinedParameter);
    }

    #[test]
    fn a_point_inverts_to_the_parameter_of_its_foot() {
        let c = line();
        let off = Vec3::new(1.0, 2.0, 0.0); // perpendicular to the direction
        let p = Point::from_vec3(c.point(-4.25).to_vec3() + off);
        let t = Curve::invert(&c, p, None, tolerance(), &mut Scratch::default()).unwrap();
        assert!((t + 4.25).abs() < 1e-12, "{t}");
        let refused = Curve::invert(
            &c,
            Point::from_vec3(Vec3::new(0.0, f64::INFINITY, 0.0)),
            None,
            tolerance(),
            &mut Scratch::default(),
        );
        assert_eq!(refused.unwrap_err().code, Code::NotFinite);
    }

    #[test]
    fn its_bounds_are_its_ends_rounded_outward() {
        let c = line();
        let b = Curve::bounds(&c, -50.0, 70.0);
        let (from, to) = (c.point(-50.0).to_vec3(), c.point(70.0).to_vec3());
        assert!((b.low().to_vec3() - Vec3::new(from.x, to.y, from.z)).norm() < 1e-12);
        assert!((b.high().to_vec3() - Vec3::new(to.x, from.y, to.z)).norm() < 1e-12);
        assert!(Curve::bounds(&c, 0.0, f64::INFINITY).contains(Point::from_vec3(Vec3::new(-1e300, 1e300, 0.0))));
    }

    #[test]
    fn its_shape_and_content_name_what_defines_it() {
        let c = line();
        let shape = Curve::shape(&c);
        assert_eq!((shape.form, shape.values.len()), ("line", 4));
        assert_eq!(shape.values[2], NamedValue::length("from", f64::NEG_INFINITY));
        assert_eq!(shape.values[0], NamedValue::point("origin", [12.0, 3.5, -7.0]));
        assert_eq!(
            shape.values[1].value,
            Value::Point({
                let d = c.first(0.0).1.rate();
                [d.x, d.y, d.z]
            })
        );
        assert_eq!(shape.values[1].quantity, Quantity::Ratio);
        let mut content = Content::default();
        Curve::content(&c, &mut content);
        assert_eq!(content.words().len(), 13, "the name's length and four letters, two vectors and the range");
        assert_eq!(Curve::family(&c), FamilyName("line"));
        assert_eq!(Curve::fit(&c), None);
        assert_eq!(Curve::range(&c), ParamRange::unbounded(ParamKind::Length));
    }
}
