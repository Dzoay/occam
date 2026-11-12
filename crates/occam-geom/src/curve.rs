//! Source: original
//! The curve seam: the surface's two layers over one parameter, [`ParametricCurve`] for a family and [`Curve`] for
//! everyone else. A curve in a face's parameters has its own two layers, in `param_curve.rs`.

use occam_math::Real;
use occam_report::Report;
use occam_units::{Bounds, Direction, Length, Point, Tangent, Tolerance, World};

use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::domain::ParamRange;
use crate::identity::{Content, FamilyName, Shape};
use crate::nearest::nearest;
use crate::support::{finite, foot_at};
use crate::surface::Closest;

/// The first layer, which a curve family implements, generic over [`Real`]. As for [`crate::Parametric`], the `eval_*`
/// methods are total and give NaN where the curve is undefined.
pub trait ParametricCurve: Sized + Send + Sync + 'static {
    /// The point.
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R>;

    /// The point and its derivative.
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>);

    /// The point and its first and second derivatives, as a surface swept from the curve needs them in lanes.
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>);

    /// Appends the derivatives of order 1 to `order` at `t`, which is inside the range, lowest first.
    ///
    /// # Errors
    ///
    /// Where the family has no derivative of an order asked for.
    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report>;

    /// Where it is defined.
    fn range(&self) -> ParamRange;

    /// A box holding the curve from `lo` to `hi`, rounding included.
    fn bounds(&self, lo: f64, hi: f64) -> Bounds;

    /// The parameter of the foot of `p`, which is finite: [`ParametricCurve::closest`]'s, unless the family has a
    /// cheaper way to it.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance, scratch: &mut Scratch) -> Result<f64, Report> {
        Ok(Self::closest(self, p, seed, tol, scratch)?.at)
    }

    /// The point of the curve, within its range, closest to `p`, which is finite. Found by a search that every family
    /// shares, within [`ParametricCurve::search`]; a family with a closed form gives that instead.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn closest(
        &self,
        p: Point<World>,
        _seed: Option<f64>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        Ok(foot_at(self, p, nearest(self, p, self.search(p))))
    }

    /// Where the foot of `p` can be, finite at both ends: the parameters [within](ParametricCurve::within) the
    /// distance from `p` of the point at the parameter in the range nearest 0, and that parameter.
    fn search(&self, p: Point<World>) -> (f64, f64) {
        let start = self.range().clamp(0.0);
        let reach = (self.eval_point(start).to_vec3() - p.to_vec3()).norm();
        holding(self.within(p, None, reach), start)
    }

    /// A finite interval inside the range holding every parameter whose point lies within `radius` of `centre`, or
    /// where `along` is given, of the line through `centre` along it; `None` where no finite interval does, as for a
    /// line that runs along `along`. A curve whose range is finite gives its range, and a family with an unbounded range
    /// narrows it: this is how a foot is searched for over a finite window, the curve's own or a swept surface's.
    fn within(&self, _centre: Point<World>, _along: Option<Direction<World>>, _radius: f64) -> Option<(f64, f64)> {
        let range = self.range();
        (range.lo.is_finite() && range.hi.is_finite()).then_some((range.lo, range.hi))
    }

    /// The family's name.
    fn family(&self) -> FamilyName;

    /// Adds the numbers that define it to `out`, which already holds the family's name.
    fn content(&self, out: &mut Content);

    /// The form and parameters a description shows.
    fn shape(&self) -> Shape;

    /// How closely a sampled curve fits what it was sampled from, which S1 found it must say (item 10). `None` for a
    /// curve that is exact.
    fn fit(&self) -> Option<Length> {
        None
    }

    /// The rational B-spline it is exactly from `lo` to `hi`, if it has one there: never where the part is unbounded,
    /// leaves the range, is empty, or is longer than a period (`docs/decisions/2026-11-11-to-bspline-over-a-range.md`).
    fn to_bspline(&self, _lo: f64, _hi: f64) -> Option<BsplineCurve> {
        None
    }
}

/// `window`, or where there is none, nothing, widened to hold `t`.
pub(crate) fn holding(window: Option<(f64, f64)>, t: f64) -> (f64, f64) {
    let (lo, hi) = window.unwrap_or((t, t));
    (if t < lo { t } else { lo }, if t > hi { t } else { hi })
}

/// The second layer for curves: dyn-compatible, and checking each call as [`crate::Surface`] does.
pub trait Curve: Send + Sync {
    /// As [`ParametricCurve::eval_point`].
    fn point(&self, t: f64) -> Point<World>;

    /// As [`ParametricCurve::eval_first`].
    fn first(&self, t: f64) -> (Point<World>, Tangent<World>);

    /// As [`ParametricCurve::eval_second`].
    fn second(&self, t: f64) -> (Point<World>, Tangent<World>, Tangent<World>);

    /// Replaces `out` with the derivatives of order 1 to `order`, lowest first.
    ///
    /// # Errors
    ///
    /// `geom.undefined-parameter` outside the range, and what [`ParametricCurve::derivatives`] reports.
    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report>;

    /// As [`ParametricCurve::closest`].
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point that is not finite, and what [`ParametricCurve::closest`] reports.
    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report>;

    /// As [`ParametricCurve::invert`].
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point that is not finite, and what [`ParametricCurve::invert`] reports.
    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance, scratch: &mut Scratch) -> Result<f64, Report>;

    /// As [`ParametricCurve::range`].
    fn range(&self) -> ParamRange;

    /// As [`ParametricCurve::bounds`].
    fn bounds(&self, lo: f64, hi: f64) -> Bounds;

    /// As [`ParametricCurve::family`].
    fn family(&self) -> FamilyName;

    /// Replaces `out` with the family's name and the numbers that define the curve.
    fn content(&self, out: &mut Content);

    /// As [`ParametricCurve::shape`].
    fn shape(&self) -> Shape;

    /// As [`ParametricCurve::fit`].
    fn fit(&self) -> Option<Length>;

    /// As [`ParametricCurve::to_bspline`].
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve>;
}

impl<C: ParametricCurve> Curve for C {
    fn point(&self, t: f64) -> Point<World> {
        self.eval_point(t)
    }

    fn first(&self, t: f64) -> (Point<World>, Tangent<World>) {
        self.eval_first(t)
    }

    fn second(&self, t: f64) -> (Point<World>, Tangent<World>, Tangent<World>) {
        self.eval_second(t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        out.clear();
        ParametricCurve::range(self).check(t)?;
        ParametricCurve::derivatives(self, t, order, out, scratch)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        finite(p)?;
        ParametricCurve::closest(self, p, seed, tol, scratch)
    }

    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance, scratch: &mut Scratch) -> Result<f64, Report> {
        finite(p)?;
        ParametricCurve::invert(self, p, seed, tol, scratch)
    }

    fn range(&self) -> ParamRange {
        ParametricCurve::range(self)
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        ParametricCurve::bounds(self, lo, hi)
    }

    fn family(&self) -> FamilyName {
        ParametricCurve::family(self)
    }

    fn content(&self, out: &mut Content) {
        out.start(ParametricCurve::family(self));
        ParametricCurve::content(self, out);
    }

    fn shape(&self) -> Shape {
        ParametricCurve::shape(self)
    }

    fn fit(&self) -> Option<Length> {
        ParametricCurve::fit(self)
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        ParametricCurve::to_bspline(self, lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use occam_units::Vec3;

    use super::*;
    use crate::conformance::{Fitted, bow, branch, endless, local, placement, ring, segment, whole_bow, whole_branch};
    use crate::line::Line;
    use crate::support::seen;

    /// Every parameter of `curve` from `lo` to `hi`, in 20,000 steps, whose point is within `radius` of `centre`, seen
    /// along `along`, lies in the curve's window for them, which is finite and inside the range. There is no window
    /// only where `refused` says there may be none.
    fn holds_every_near_parameter<C: ParametricCurve>(
        curve: &C,
        (lo, hi): (f64, f64),
        (centre, along, radius): (Point<World>, Option<Direction<World>>, f64),
        refused: bool,
    ) {
        let (range, window) = (curve.range(), curve.within(centre, along, radius));
        let Some((from, to)) = window else {
            assert!(refused, "no window about {centre:?} along {along:?}");
            return;
        };
        assert!(range.contains(from) && range.contains(to), "{window:?} leaves {range:?}");
        let near = (0..=20_000)
            .map(|k| lo + (hi - lo) * f64::from(k) / 20_000.0)
            .filter(|t| range.contains(*t))
            .filter(|t| seen(curve.eval_point(*t).to_vec3() - centre.to_vec3(), along).norm() <= radius);
        for t in near {
            assert!(from <= t && t <= to, "{t} is within {radius} of {centre:?} along {along:?}, outside {window:?}");
        }
    }

    /// The hyperbola's first asymptote, along which it runs as its parameter rises.
    fn asymptote() -> Direction<World> {
        Direction::normalised(whole_branch().eval_first(30.0).1.rate())
    }

    /// Centres near the curves and far from them, directions none, slanting, along the parabola's axis and along the
    /// hyperbola's asymptote, and radii from small to large.
    fn cases() -> Vec<(Point<World>, Option<Direction<World>>, f64)> {
        let centres = [local(7.0, 1.0, -2.0), local(30.0, -20.0, 15.0), local(-5.0, 4.0, 0.0), local(0.0, 0.0, 200.0)];
        let slant = placement().direction(Direction::normalised(Vec3::new(0.3, -0.2, 1.0)));
        let axis = Direction::normalised(whole_bow().eval_second(0.0).2.rate());
        let alongs = [None, Some(slant), Some(axis), Some(asymptote())];
        let mut out = vec![];
        for centre in centres {
            for along in alongs {
                for radius in [0.5, 4.0, 60.0] {
                    out.push((Point::from_vec3(centre), along, radius));
                }
            }
        }
        out
    }

    #[test]
    fn a_window_holds_every_parameter_that_comes_within_reach() {
        for case in cases() {
            let along_asymptote = case.1 == Some(asymptote());
            holds_every_near_parameter(&endless(), (-300.0, 300.0), case, false);
            holds_every_near_parameter(&segment(), (-300.0, 300.0), case, false);
            holds_every_near_parameter(&whole_bow(), (-120.0, 120.0), case, false);
            holds_every_near_parameter(&bow(), (-120.0, 120.0), case, false);
            holds_every_near_parameter(&whole_branch(), (-14.0, 40.0), case, along_asymptote);
            holds_every_near_parameter(&branch(), (-14.0, 40.0), case, false);
            holds_every_near_parameter(&whole_branch().trimmed(-14.0, 40.0).unwrap(), (-14.0, 40.0), case, false);
            holds_every_near_parameter(&ring(), (0.0, std::f64::consts::TAU), case, false);
        }
    }

    #[test]
    fn no_window_holds_a_line_or_an_unbounded_branch_seen_along_itself() {
        let line = endless();
        let own = Direction::normalised(line.eval_first(0.0).1.rate());
        let centre = Point::from_vec3(local(1.0, 2.0, 3.0));
        assert_eq!(line.within(centre, Some(own), 5.0), None);
        assert!(line.within(centre, None, 5.0).is_some());
        let (lo, hi) = segment().search(centre);
        assert!(lo >= -3.0 && hi <= 5.0 && lo <= hi, "inside the segment: {lo} {hi}");
        assert_eq!(whole_branch().within(centre, Some(asymptote()), 5.0), None);
        let rising = whole_branch().trimmed(-300.0, 2.0).unwrap();
        assert!(rising.within(centre, Some(asymptote()), 5.0).is_some(), "it does not run off that way");
    }

    #[test]
    fn the_object_layer_passes_on_a_fit() {
        let line =
            Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let c: &dyn Curve = &Fitted(line);
        assert_eq!(c.fit(), Some(Length::from_millimetres(0.25)));
        assert_eq!(c.family(), FamilyName("fitted"));
    }

    /// A ray: a line's parameter from 0 without end, through the defaults for its search.
    struct Ray(Line);

    impl ParametricCurve for Ray {
        fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
            self.0.eval_point(t)
        }

        fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
            self.0.eval_first(t)
        }

        fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
            self.0.eval_second(t)
        }

        fn derivatives(
            &self,
            t: f64,
            order: u8,
            out: &mut Vec<Tangent<World>>,
            scratch: &mut Scratch,
        ) -> Result<(), Report> {
            ParametricCurve::derivatives(&self.0, t, order, out, scratch)
        }

        fn range(&self) -> ParamRange {
            ParamRange { lo: 0.0, ..ParametricCurve::range(&self.0) }
        }

        fn bounds(&self, lo: f64, hi: f64) -> Bounds {
            ParametricCurve::bounds(&self.0, lo, hi)
        }

        fn family(&self) -> FamilyName {
            FamilyName("ray")
        }

        fn content(&self, out: &mut Content) {
            ParametricCurve::content(&self.0, out);
        }

        fn shape(&self) -> Shape {
            ParametricCurve::shape(&self.0)
        }
    }

    fn x_line_through(x: f64) -> Line {
        Line::new(Point::from_vec3(Vec3::new(x, 0.0, 0.0)), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)))
    }

    #[test]
    fn a_range_with_one_end_unbounded_has_no_finite_window_by_default() {
        let ray = Ray(x_line_through(0.0));
        assert_eq!(ParametricCurve::within(&ray, Point::from_vec3(Vec3::new(1.0, 1.0, 0.0)), None, 1.0), None);
    }

    #[test]
    fn a_search_reaches_as_far_as_the_point_at_the_parameter_nearest_zero() {
        // The line through (1, 0, 0) along X: its point at 0 is (1, 0, 0), the square root of 20 from (3, 4, 0), and
        // the line's window is that either side of the parameter nearest the point, 2.
        let line = x_line_through(1.0);
        let (lo, hi) = ParametricCurve::search(&line, Point::from_vec3(Vec3::new(3.0, 4.0, 0.0)));
        let reach = 20f64.sqrt();
        assert!((lo - (2.0 - reach)).abs() < 1e-14 && (hi - (2.0 + reach)).abs() < 1e-14, "{lo} {hi}");
    }

    #[test]
    fn a_window_is_widened_to_hold_its_anchor() {
        assert_eq!(holding(Some((0.0, 4.0)), 6.0), (0.0, 6.0));
        assert_eq!(holding(Some((0.0, 4.0)), -1.0), (-1.0, 4.0));
        assert_eq!(holding(Some((0.0, 4.0)), 1.0), (0.0, 4.0));
        assert_eq!(holding(None, 2.5), (2.5, 2.5));
    }
}
