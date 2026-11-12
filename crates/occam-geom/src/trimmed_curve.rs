//! Source: original
//! The trimmed curve: any curve restricted to a part of its range (PARITY 2.1), for a family that has no trim of its
//! own, its range as [`ParamRange::trimmed`] makes it, and everything else the base's. Its foot is the base's where
//! that lies in the part, as the nearest point of the whole is then the nearest of the part, and otherwise the curves'
//! shared search over the part.

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Point, Tangent, Tolerance, World};

use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::identity::{Content, FamilyName, Shape};
use crate::nearest::nearest;
use crate::support::foot_at;
use crate::surface::Closest;
use crate::swept::{push_curve, with_curve};
use crate::trimmed_surface::into;

/// A curve restricted to a part of its range.
#[derive(Clone, Debug, PartialEq)]
pub struct TrimmedCurve<C> {
    base: C,
    range: ParamRange,
}

impl<C: ParametricCurve> TrimmedCurve<C> {
    /// `base` from `lo` to `hi`.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`]: an end outside the base's range, not finite, `hi` not above `lo`, or more than a
    /// period of a range that comes round.
    pub fn new(base: C, lo: f64, hi: f64) -> Result<Self, Report> {
        let range = base.range().trimmed(lo, hi)?;
        Ok(Self { base, range })
    }
}

impl<C: ParametricCurve> ParametricCurve for TrimmedCurve<C> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        self.base.eval_point(t)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        self.base.eval_first(t)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        self.base.eval_second(t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        self.base.derivatives(t, order, out, scratch)
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        self.base.bounds(lo, hi)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        let whole = self.base.closest(p, seed, tol, scratch)?;
        let t = into(self.base.range(), self.range, whole.at);
        let at = if self.range.contains(t) { t } else { nearest(self, p, self.search(p)) };
        Ok(foot_at(self, p, at))
    }

    fn family(&self) -> FamilyName {
        FamilyName("trimmed")
    }

    fn content(&self, out: &mut Content) {
        out.push(self.range.lo);
        out.push(self.range.hi);
        push_curve(out, &self.base);
    }

    fn shape(&self) -> Shape {
        let ends = vec![
            NamedValue::new("from", self.range.quantity(), self.range.lo),
            NamedValue::new("to", self.range.quantity(), self.range.hi),
        ];
        Shape { form: "trimmed", values: with_curve(ends, &self.base) }
    }

    fn fit(&self) -> Option<Length> {
        self.base.fit()
    }

    /// The base's B-spline over a part inside this one.
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        self.range.trimmed(lo, hi).ok()?;
        self.base.to_bspline(lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use occam_report::Code;
    use occam_units::{Direction, Vec3};

    use super::*;
    use crate::conformance::{self, Check, Fitted, WrongSecondCurve, local, placement, tolerance, twist, whole_branch};
    use crate::curve::Curve;
    use crate::{Circle, Line};

    fn circle() -> Circle {
        Circle::placed(&placement(), Length::from_millimetres(10.0)).unwrap()
    }

    fn cut<C: ParametricCurve>(base: C, lo: f64, hi: f64) -> TrimmedCurve<C> {
        TrimmedCurve::new(base, lo, hi).unwrap()
    }

    fn passes(c: &dyn Curve) {
        let range = c.range();
        let failures = conformance::curve(c, range.lo, range.hi);
        assert!(failures.is_empty(), "{:?}: {failures:#?}", c.shape().values);
    }

    #[test]
    fn a_trimmed_curve_of_every_kind_passes_the_suite() {
        let range = Curve::range(&twist());
        passes(&cut(twist(), range.lo + 0.2 * (range.hi - range.lo), range.lo + 0.7 * (range.hi - range.lo)));
        passes(&cut(circle(), 5.0, 7.0));
        passes(&cut(circle(), 1.0, 1.0 + TAU));
        passes(&cut(whole_branch(), -1.5, 2.0));
        let line = Line::new(Point::from_vec3(local(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(0.0, 1.0, 1.0)));
        passes(&cut(line, -4.0, 6.5));
    }

    #[test]
    fn the_suite_catches_a_trimmed_curve_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&WrongSecondCurve(cut(circle(), 5.0, 7.0)), 5.0, 7.0));
        assert_eq!(failed, [Check::Differences]);
    }

    #[test]
    fn its_foot_is_the_bases_in_the_part_and_the_nearest_end_or_valley_outside() {
        let (c, tol) = (cut(circle(), 5.0, 7.0), tolerance());
        let on = c.point(6.5);
        assert!((Curve::invert(&c, on, None, tol, &mut Scratch::default()).unwrap() - 6.5).abs() < 1e-12);
        let past_seam = c.point(TAU + 0.3);
        let found = Curve::invert(&c, past_seam, None, tol, &mut Scratch::default()).unwrap();
        assert!((found - (TAU + 0.3)).abs() < 1e-12, "the base's 0.3, a turn on: {found}");
        let opposite = Point::from_vec3(local(-12.0, 1.0, 0.0));
        let end = Curve::closest(&c, opposite, None, tol, &mut Scratch::default()).unwrap();
        assert!((end.at - 5.0).abs() < 1e-9 || (end.at - 7.0).abs() < 1e-9, "{end:?}");
        let other = if (end.at - 5.0).abs() < 1e-9 { 7.0 } else { 5.0 };
        assert!(end.distance.millimetres() <= (c.point(other).to_vec3() - opposite.to_vec3()).norm());
    }

    #[test]
    fn a_part_outside_the_range_or_back_to_front_is_refused() {
        let arc = circle().trimmed(0.5, 2.0).unwrap();
        for (lo, hi) in [(0.0, 1.0), (1.5, 1.0), (1.0, 2.5)] {
            assert_eq!(TrimmedCurve::new(arc, lo, hi).unwrap_err().code, Code::UndefinedParameter, "{lo} {hi}");
        }
    }

    #[test]
    fn it_presents_the_bases_bspline_inside_the_part_and_names_what_defines_it() {
        let c = cut(circle(), 5.0, 7.0);
        assert_eq!(Curve::to_bspline(&c, 5.5, 6.5), Curve::to_bspline(&circle(), 5.5, 6.5));
        assert_eq!(Curve::to_bspline(&c, 4.0, 6.5), None, "past the part");
        assert!(conformance::presented_curve(&c, 5.0, 7.0).is_empty());
        let shape = Curve::shape(&c);
        let names: Vec<&str> = shape.values.iter().take(3).map(|v| v.name.as_str()).collect();
        assert_eq!((shape.form, names), ("trimmed", vec!["from", "to", "curve-centre"]));
        let mut content = Content::default();
        Curve::content(&c, &mut content);
        assert_eq!(&content.words()[8..10], [5f64, 7.0].map(f64::to_bits));
        assert_eq!((Curve::family(&c), Curve::fit(&c)), (FamilyName("trimmed"), None));
    }

    #[test]
    fn a_trimmed_curve_passes_its_base_s_fit_on() {
        let x = Direction::normalised(Vec3::new(1.0, 0.0, 0.0));
        let part =
            TrimmedCurve::new(Fitted(Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), x)), 0.0, 1.0).unwrap();
        assert_eq!(ParametricCurve::fit(&part), Some(Length::from_millimetres(0.25)));
    }
}
