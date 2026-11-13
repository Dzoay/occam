//! Source: original
//! The circle: `centre + r cos t x + r sin t y`, where `t` is an angle that comes round, whole or trimmed to an arc
//! (PARITY 1.4). Its foot is closed-form: the angle of the point in the circle's plane.

use occam_math::{Real, atan2};
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Local, Placement, Point, Tangent, Tolerance, World};

use crate::axes::Axes;
use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::exact::{self, Conic};
use crate::identity::{Content, FamilyName, Shape};
use crate::support::{foot_at, positive};
use crate::surface::Closest;
use crate::turn::{above, arc_box, on_turn, on_turn_first, on_turn_second, turn, turn_derivatives};

/// A circle, or an arc of one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    axes: Axes,
    radius: f64,
    range: ParamRange,
}

impl Circle {
    /// The circle of `radius` about the origin of `placement`, in its XY plane, from its X axis towards its Y.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length greater than zero.
    pub fn placed(placement: &Placement<Local, World>, radius: Length) -> Result<Self, Report> {
        Ok(Self { axes: Axes::of(placement), radius: positive(radius)?, range: turn() })
    }

    /// The arc from angle `lo` to `hi`.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`].
    pub fn trimmed(self, lo: f64, hi: f64) -> Result<Self, Report> {
        Ok(Self { range: self.range.trimmed(lo, hi)?, ..self })
    }

    /// The parameter of `p`'s foot: its angle about the centre, or on an arc that does not reach it, the nearer end, the
    /// start where both are as near.
    fn foot(&self, p: Point<World>) -> f64 {
        let (u, v) = self.axes.coordinates(p);
        let angle = above(atan2(v, u), self.range.lo);
        if self.range.contains(angle) {
            return angle;
        }
        let gap = |t: f64| (self.eval_point(t).to_vec3() - p.to_vec3()).norm();
        if gap(self.range.hi) < gap(self.range.lo) { self.range.hi } else { self.range.lo }
    }
}

impl ParametricCurve for Circle {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        on_turn(&self.axes, (self.radius, self.radius), t)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        on_turn_first(&self.axes, (self.radius, self.radius), t)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        on_turn_second(&self.axes, (self.radius, self.radius), t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        turn_derivatives(&self.axes, (self.radius, self.radius), t, order, out);
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let ends = (self.eval_point(lo).to_vec3(), self.eval_point(hi).to_vec3());
        arc_box(&self.axes, (self.radius, self.radius), (lo, hi), ends)
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<f64>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<f64, Report> {
        Ok(self.foot(p))
    }

    fn closest(
        &self,
        p: Point<World>,
        _seed: Option<f64>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        Ok(foot_at(self, p, self.foot(p)))
    }

    fn family(&self) -> FamilyName {
        FamilyName("circle")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push_curve(out, &[self.radius], self.range);
    }

    fn shape(&self) -> Shape {
        Shape {
            form: "circle",
            values: self.axes.curve_values("centre", &[NamedValue::length("radius", self.radius)], self.range),
        }
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        exact::conic_curve(Conic::Elliptic, (&self.axes, (self.radius, self.radius)), self.range, (lo, hi))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use occam_report::{Code, Quantity};
    use occam_units::Vec3;

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, tolerance};
    use crate::curve::Curve;

    fn circle() -> Circle {
        Circle::placed(&placement(), Length::from_millimetres(10.0)).unwrap()
    }

    #[test]
    fn the_circle_and_its_arcs_pass_the_conformance_suite() {
        for (lo, hi, whole) in [(0.0, TAU, true), (0.5, 2.5, false), (5.0, 7.0, false), (1.0, 1.0 + TAU, false)] {
            let c = if whole { circle() } else { circle().trimmed(lo, hi).unwrap() };
            let failures = conformance::curve(&c, lo, hi);
            assert!(failures.is_empty(), "{lo} {hi}: {failures:#?}");
        }
        assert_eq!(Curve::range(&circle().trimmed(1.0, 1.0 + TAU).unwrap()).wrap, crate::Wrap::Closed);
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let c = circle();
        assert!(agree(c.point(0.0).to_vec3(), local(10.0, 0.0, 0.0)));
        assert!(agree(c.point(FRAC_PI_2).to_vec3(), local(0.0, 10.0, 0.0)));
        assert!(agree(c.point(PI).to_vec3(), local(-10.0, 0.0, 0.0)));
        let along = |x: f64, y: f64| local(x, y, 0.0) - local(0.0, 0.0, 0.0);
        let mut out = vec![];
        Curve::derivatives(&c, 0.0, 5, &mut out, &mut Scratch::default()).unwrap();
        let want = [along(0.0, 10.0), along(-10.0, 0.0), along(0.0, -10.0), along(10.0, 0.0), along(0.0, 10.0)];
        for (got, want) in out.iter().zip(want) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
        assert_eq!((out.len(), c.first(0.0).1), (5, out[0]));
    }

    #[test]
    fn the_foot_is_the_angle_in_the_plane_or_the_nearer_end() {
        let c = circle();
        let tol = tolerance();
        let above = Point::from_vec3(local(3.0, 3.0, 7.0));
        let foot = Curve::closest(&c, above, None, tol, &mut Scratch::default()).unwrap();
        assert!(
            (foot.at - PI / 4.0).abs() < 1e-14 && agree(foot.point.to_vec3(), local(50f64.sqrt(), 50f64.sqrt(), 0.0))
        );
        let centre = Point::from_vec3(local(0.0, 0.0, 2.0));
        let any = Curve::closest(&c, centre, None, tol, &mut Scratch::default()).unwrap();
        assert!((any.distance.millimetres() - 104f64.sqrt()).abs() < 1e-12, "every point is as near: {any:?}");
        let behind = Point::from_vec3(local(-1.0, -1.0, 0.0));
        let below = Curve::invert(&c, behind, None, tol, &mut Scratch::default()).unwrap();
        assert!((below - 5.0 * PI / 4.0).abs() < 1e-14, "{below}: a whole circle's angle is from 0 to 2π");
        let arc = c.trimmed(0.5, 2.5).unwrap();
        let near_start = Point::from_vec3(local(10.0, -1.0, 0.0));
        let near_end = Point::from_vec3(local(-10.0, 1.0, 0.0));
        assert_eq!(Curve::invert(&arc, near_start, None, tol, &mut Scratch::default()).unwrap(), 0.5);
        assert_eq!(Curve::invert(&arc, near_end, None, tol, &mut Scratch::default()).unwrap(), 2.5);
        let across_seam = c.trimmed(5.0, 7.0).unwrap();
        let t =
            Curve::invert(&across_seam, Point::from_vec3(local(10.0, 0.5, 0.0)), None, tol, &mut Scratch::default());
        assert!((t.unwrap() - (TAU + occam_math::atan2(0.5, 10.0))).abs() < 1e-12, "past the seam, within the arc");
    }

    #[test]
    fn of_two_ends_as_near_the_foot_is_the_start() {
        // An arc about angle π, and a point far along angle 0: the ends' distances round to the same double.
        let flat = Circle::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap();
        let arc = flat.trimmed(PI - 1.0, PI + 1.0).unwrap();
        let far = Point::from_vec3(Vec3::new(1000.0, 0.0, 0.0));
        let gap = |t: f64| (arc.point(t).to_vec3() - far.to_vec3()).norm();
        assert_eq!(gap(PI - 1.0), gap(PI + 1.0), "the ends are exactly as near");
        assert_eq!(Curve::invert(&arc, far, None, tolerance(), &mut Scratch::default()).unwrap(), PI - 1.0);
    }

    #[test]
    fn an_arc_box_reaches_the_extremes_the_arc_holds_and_no_further() {
        let flat = Circle::placed(&Placement::identity(), Length::from_millimetres(10.0)).unwrap();
        let root = 50f64.sqrt();
        for ((lo, hi), low, high) in [
            ((0.0, FRAC_PI_2), (0.0, 0.0), (10.0, 10.0)),
            ((PI / 4.0, 3.0 * PI / 4.0), (-root, root), (root, 10.0)),
            ((PI, 1.5 * PI), (-10.0, -10.0), (0.0, 0.0)),
            ((0.0, TAU), (-10.0, -10.0), (10.0, 10.0)),
            ((5.0, 5.0 + TAU), (-10.0, -10.0), (10.0, 10.0)),
        ] {
            let b = Curve::bounds(&flat, lo, hi);
            let (l, h) = (b.low().to_vec3(), b.high().to_vec3());
            let near = |got: f64, want: f64| got <= want + 1e-14 && got >= want - 1e-12;
            assert!(
                near(l.x, low.0) && near(l.y, low.1) && near(-h.x, -high.0) && near(-h.y, -high.1),
                "{lo} {hi}: {b:?}"
            );
            assert!(l.x <= low.0 && l.y <= low.1 && h.x >= high.0 && h.y >= high.1 && l.z < 0.0 && h.z > 0.0);
        }
        assert!(Curve::bounds(&flat, 0.0, f64::INFINITY).contains(Point::from_vec3(Vec3::new(1e300, 0.0, 0.0))));
    }

    #[test]
    fn a_radius_must_be_positive_and_an_arc_inside_a_turn() {
        for bad in [0.0, -1.0, f64::NAN] {
            let refused = Circle::placed(&placement(), Length::from_millimetres(bad)).unwrap_err();
            assert_eq!(refused.code, Code::SizeNotPositive);
        }
        assert_eq!(circle().trimmed(1.0, 8.0).unwrap_err().code, Code::UndefinedParameter);
    }

    #[test]
    fn its_shape_and_content_name_what_defines_it() {
        let arc = circle().trimmed(0.5, 2.5).unwrap();
        let shape = Curve::shape(&arc);
        assert_eq!((shape.form, shape.values.len()), ("circle", 6));
        assert_eq!(shape.values[3], NamedValue::length("radius", 10.0));
        assert_eq!(shape.values[5], NamedValue::new("to", Quantity::Angle, 2.5));
        let mut content = Content::default();
        Curve::content(&arc, &mut content);
        assert_eq!(content.words().len(), 22, "the name, the axes, the radius and the range");
        assert_eq!(content.words().last(), Some(&2.5f64.to_bits()));
    }

    #[test]
    fn the_suite_catches_a_circle_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&conformance::WrongSecondCurve(circle()), 0.0, TAU));
        assert_eq!(failed, [Check::Differences]);
    }
}
