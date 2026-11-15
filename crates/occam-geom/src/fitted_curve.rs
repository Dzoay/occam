//! Source: original
//! The fitted curve: any curve with how closely it fits what it was fitted to, as a section curve fitted to traced points
//! must say (S1 item 10), and everything else the base's. A fit is a length; drawn in a face's parameters through
//! `UvCurve`, it is in the parameters' own measure. Moved by a map, the fit grows as much as the map stretches any
//! length, since that is as far as the curve and what it was fitted to can move apart.

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Affine, Bounds, Direction, Length, Point, Tangent, Tolerance, World};

use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::ParametricCurve;
use crate::domain::{ParamMap, ParamRange};
use crate::identity::{Content, FamilyName, Shape};
use crate::support::not_negative;
use crate::surface::Closest;
use crate::swept::{push_curve, with_curve};

/// A curve that was fitted, with how closely it fits.
#[derive(Clone, Debug, PartialEq)]
pub struct FittedCurve<C> {
    curve: C,
    /// The fit, in millimetres, never below zero.
    fit: f64,
}

impl<C: ParametricCurve> FittedCurve<C> {
    /// `base`, which fits what it was fitted to within `fit`.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a fit below zero or not finite.
    pub fn new(base: C, fit: Length) -> Result<Self, Report> {
        Ok(Self { curve: base, fit: not_negative(fit)? })
    }
}

impl<C: ParametricCurve> ParametricCurve for FittedCurve<C> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        self.curve.eval_point(t)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        self.curve.eval_first(t)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        self.curve.eval_second(t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        self.curve.derivatives(t, order, out, scratch)
    }

    fn range(&self) -> ParamRange {
        self.curve.range()
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        self.curve.bounds(lo, hi)
    }

    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance, scratch: &mut Scratch) -> Result<f64, Report> {
        self.curve.invert(p, seed, tol, scratch)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        self.curve.closest(p, seed, tol, scratch)
    }

    fn search(&self, p: Point<World>) -> (f64, f64) {
        self.curve.search(p)
    }

    fn within(&self, centre: Point<World>, along: Option<Direction<World>>, radius: f64) -> Option<(f64, f64)> {
        self.curve.within(centre, along, radius)
    }

    fn family(&self) -> FamilyName {
        FamilyName("fitted")
    }

    fn content(&self, out: &mut Content) {
        out.push(self.fit);
        push_curve(out, &self.curve);
    }

    fn shape(&self) -> Shape {
        Shape { form: "fitted", values: with_curve(vec![NamedValue::length("fit", self.fit)], &self.curve) }
    }

    fn fit(&self) -> Option<Length> {
        Some(Length::from_millimetres(self.fit))
    }

    /// The base's: the curve is that B-spline exactly, however closely it fits what it was fitted to.
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        self.curve.to_bspline(lo, hi)
    }

    /// A map keeps a fitted curve where it keeps its base, its fit grown by the most the map stretches a length.
    fn moved(&self, map: &Affine) -> Option<(Self, ParamMap)> {
        let (curve, params) = self.curve.moved(map)?;
        Some((Self { curve, fit: self.fit * map.most() }, params))
    }
}

#[cfg(test)]
mod tests {
    use occam_report::Code;
    use occam_units::{Placement, Vec3, Vector};

    use super::*;
    use crate::conformance::{self, Check, WrongSecondCurve, placement, twist};
    use crate::curve::Curve;
    use crate::param_curve::ParamCurve;
    use crate::{Circle, Line, SplineCurve, UvCurve};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn fitted<C: ParametricCurve>(base: C, fit: f64) -> FittedCurve<C> {
        FittedCurve::new(base, mm(fit)).unwrap()
    }

    fn circle() -> Circle {
        Circle::placed(&placement(), mm(10.0)).unwrap()
    }

    /// A cubic in the plane `z = 0`, as a curve in a face's parameters is drawn.
    fn flat() -> SplineCurve {
        let at = |x: f64, y: f64| Point::from_vec3(Vec3::new(x, y, 0.0));
        let poles = vec![at(0.0, 0.0), at(1.0, 2.0), at(3.0, 1.5), at(4.0, -1.0), at(6.0, 0.5)];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0];
        SplineCurve::new(&BsplineCurve { degree: 3, knots, poles, weights: None }).unwrap()
    }

    fn passes(c: &dyn Curve) {
        let range = c.range();
        let failures = conformance::curve(c, range.lo, range.hi);
        assert!(failures.is_empty(), "{:?}: {failures:#?}", c.shape().values);
    }

    #[test]
    fn a_fitted_curve_of_every_kind_passes_the_suite() {
        passes(&fitted(twist(), 1e-4));
        passes(&fitted(circle(), 0.0));
        let range = Curve::range(&twist());
        let failures = conformance::curve(&fitted(twist(), 2e-3), range.lo + 0.1, range.hi - 0.2);
        assert!(failures.is_empty(), "a part: {failures:#?}");
        let image = UvCurve::new(fitted(flat(), 1e-6)).unwrap();
        let failures = conformance::param_curve(&image, 0.0, 1.0);
        assert!(failures.is_empty(), "in a face's parameters: {failures:#?}");
    }

    #[test]
    fn the_suite_catches_a_fitted_curve_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&WrongSecondCurve(fitted(circle(), 0.1)), 1.0, 4.0));
        assert_eq!(failed, [Check::Differences]);
    }

    #[test]
    fn it_says_its_fit_and_in_a_faces_parameters_says_it_there() {
        let c = fitted(twist(), 0.25);
        assert_eq!((Curve::fit(&c), Curve::fit(&twist())), (Some(mm(0.25)), None));
        assert_eq!(Curve::family(&c), FamilyName("fitted"));
        let image = UvCurve::new(fitted(flat(), 0.125)).unwrap();
        assert_eq!(ParamCurve::fit(&image), Some(0.125));
        assert_eq!(fitted(circle(), -0.0).fit.to_bits(), 0.0_f64.to_bits(), "zero of either sign is one fit");
    }

    #[test]
    fn a_fit_below_zero_or_not_finite_is_refused() {
        for bad in [-1e-9, f64::NAN, f64::INFINITY] {
            assert_eq!(FittedCurve::new(circle(), mm(bad)).unwrap_err().code, Code::SizeNotPositive, "{bad}");
        }
    }

    #[test]
    fn its_content_and_shape_hold_the_fit_and_then_the_base() {
        let (c, other) = (fitted(circle(), 0.5), fitted(circle(), 0.75));
        let (mut words, mut base, mut again) = (Content::default(), Content::default(), Content::default());
        Curve::content(&c, &mut words);
        Curve::content(&circle(), &mut base);
        Curve::content(&other, &mut again);
        assert_eq!(words.words().get(words.words().len() - base.words().len()..), Some(base.words()));
        assert_ne!(words, again, "two fits are two curves");
        let shape = Curve::shape(&c);
        let names: Vec<&str> = shape.values.iter().map(|v| v.name.as_str()).collect();
        assert_eq!((shape.form, names.first(), shape.values[0].number()), ("fitted", Some(&"fit"), Some(0.5)));
        assert!(names.iter().skip(1).all(|name| name.starts_with("curve-")), "{names:?}");
    }

    #[test]
    fn it_searches_for_a_foot_over_its_bases_window() {
        // A line has a window of its own for each point and reach, none of them a constant one.
        let along = Direction::normalised(Vec3::new(1.0, 2.0, -0.5));
        let line = || Line::new(Point::from_vec3(Vec3::new(3.0, -1.0, 2.0)), along);
        let c = fitted(line(), 1e-3);
        for t in [50.0, -20.0] {
            let at = Curve::point(&line(), t).to_vec3() + Vec3::new(0.1, -0.05, 0.2);
            let p = Point::from_vec3(at);
            assert_eq!(ParametricCurve::search(&c, p), ParametricCurve::search(&line(), p), "{at:?}");
            for (across, radius) in [(None, 2.0), (Some(Direction::normalised(Vec3::new(0.0, 0.0, 1.0))), 5.0)] {
                let window = ParametricCurve::within(&c, p, across, radius);
                assert_eq!(window, ParametricCurve::within(&line(), p, across, radius));
                assert!(window.is_some_and(|(lo, hi)| lo.abs() > 1.0 && hi.abs() > 1.0), "{window:?}");
            }
        }
    }

    #[test]
    fn moved_its_fit_grows_by_the_most_the_map_stretches() {
        let c = fitted(twist(), 0.5);
        let scaled = Affine::similarity(Placement::translation(Vector::from_vec3(Vec3::new(1.0, 2.0, 3.0))), 3.0);
        let stretch = Affine::stretch(&placement(), [2.0, 0.5, 4.0]);
        for (map, grown) in
            [(Affine::placed(Placement::identity()), 0.5), (scaled.unwrap(), 1.5), (stretch.unwrap(), 2.0)]
        {
            let (moved, params) = Curve::moved(&c, &map).unwrap();
            assert_eq!((moved.fit(), params), (Some(mm(grown)), ParamMap::IDENTITY));
            assert!((moved.point(0.3).to_vec3() - map.point(c.point(0.3)).to_vec3()).norm() < 1e-12);
        }
        assert_eq!(Curve::moved(&fitted(circle(), 0.5), &stretch.unwrap()).map(|(m, _)| m.fit()), None);
        let mut scratch = Scratch::default();
        let near = Point::from_vec3(c.point(0.4).to_vec3() + Vec3::new(0.0, 0.0, 1e-3));
        let tol = conformance::tolerance();
        assert_eq!(
            Curve::invert(&c, near, None, tol, &mut scratch),
            Curve::invert(&twist(), near, None, tol, &mut scratch)
        );
    }
}
