//! Source: original
//! Every kind of parameter curve from the curve families in space: a curve drawn in the plane `z = 0`, as a placement
//! with no tilt draws it, is read as `(u, v) = (x, y)`. The reading is a linear map, so its points, its derivatives to
//! any order, its box, its window for a search and its B-spline carry over exactly, and a line, a circle, a conic, a
//! B-spline or Bézier, an offset or a trimmed curve in a face's parameters is that family, with no second copy of its
//! formulas, and its shape and content are the parameter curve's. A curve that leaves the plane is refused, so that what
//! it says it is, it is in the parameters too: a tilted circle's shadow would be an ellipse that called itself a circle.
//!
//! The parameters' units are read off the plane's millimetres: one millimetre there is one unit of each parameter, a
//! radian on an angle, a millimetre on a length, and a knot as itself. A fit, likewise, is in the parameters' measure.

use occam_math::Real;
use occam_report::{Code, Fault, NamedValue, Report};
use occam_units::{Direction, Point, Tangent, UvPoint, UvVector, Vec3, World};

use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::{ParametricCurve, holding};
use crate::domain::{ParamRange, UvBox};
use crate::identity::{Content, FamilyName, Shape};
use crate::nearest::nearest_by;
use crate::param_curve::{ParametricParamCurve, UvClosest};

/// A curve in a face's parameters: the curve `C` read in its XY plane.
#[derive(Clone, Debug, PartialEq)]
pub struct UvCurve<C> {
    curve: C,
}

/// The point of the plane at `uv`.
fn lifted(uv: UvPoint) -> Point<World> {
    Point::from_vec3(Vec3::new(uv.u(), uv.v(), 0.0))
}

/// A point's place in the plane.
#[inline(always)]
fn place<R: Real>(p: Point<World, R>) -> UvPoint<R> {
    let v = p.to_vec3();
    UvPoint::new(v.x, v.y)
}

/// A rate's part in the plane.
#[inline(always)]
fn along<R: Real>(t: Tangent<World, R>) -> UvVector<R> {
    let v = t.rate();
    UvVector::new(v.x, v.y)
}

/// A vector's shadow on the plane, as a vector in space.
fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

/// How far, in machine epsilons of its size, a box may reach from the plane and still show its curve on it. A box holds
/// its curve, rounding included, so this is the most padding a curve family puts on one: a degree-25 spline's 64 for
/// each of its 27 sums, 1,728, and an offset's 512 on top of that, 2,240. A curve that truly leaves the plane reaches
/// further, by more than four thousand epsilons of its size.
const SLACK: f64 = 4096.0;

/// The part of `range` whose box decides the plane: all of it where it is finite. A range without end is a line's, a
/// parabola's, a hyperbola's or an offset of one, each analytic over all of it, so a stretch of two units, at its
/// finite end or about 0, holds distinct points enough to decide its plane.
fn stretch(range: ParamRange) -> (f64, f64) {
    match (range.lo.is_finite(), range.hi.is_finite()) {
        (true, true) => (range.lo, range.hi),
        (true, false) => (range.lo, range.lo + 2.0),
        (false, true) => (range.hi - 2.0, range.hi),
        (false, false) => (-1.0, 1.0),
    }
}

impl<C: ParametricCurve> UvCurve<C> {
    /// The parameter curve that `curve` draws in the plane `z = 0`, `(u, v) = (x, y)`.
    ///
    /// # Errors
    ///
    /// `geom.not-in-plane` for a curve that leaves the plane, decided by its own box over its range, or over a stretch
    /// of it where it has no end, which holds it: one that reaches no further from the plane than rounding shows the
    /// curve on it, and one that reaches further, by its largest `height`, refuses it.
    pub fn new(curve: C) -> Result<Self, Report> {
        let (lo, hi) = stretch(curve.range());
        let b = curve.bounds(lo, hi);
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        let size = [low.x, low.y, low.z, high.x, high.y, high.z]
            .into_iter()
            .fold(0.0, |a, x| if x.abs() > a { x.abs() } else { a });
        let height = if high.z.abs() > low.z.abs() { high.z.abs() } else { low.z.abs() };
        let on_plane = height.is_finite() && height <= SLACK * f64::EPSILON * size;
        if on_plane {
            return Ok(Self { curve });
        }
        Err(Report::new(Code::NotInPlane, Fault::InvalidInput).with_value(NamedValue::length("height", height)))
    }
}

impl<C: ParametricCurve> ParametricParamCurve for UvCurve<C> {
    #[inline(always)]
    fn eval_uv<R: Real>(&self, t: R) -> UvPoint<R> {
        place(self.curve.eval_point(t))
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>) {
        let (point, first) = self.curve.eval_first(t);
        (place(point), along(first))
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>, UvVector<R>) {
        let (point, first, second) = self.curve.eval_second(t);
        (place(point), along(first), along(second))
    }

    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<UvVector>, scratch: &mut Scratch) -> Result<(), Report> {
        let mut rates = Vec::with_capacity(usize::from(order));
        self.curve.derivatives(t, order, &mut rates, scratch)?;
        out.extend(rates.into_iter().map(along));
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.curve.range()
    }

    /// The curve's box, read in the plane.
    fn bounds(&self, lo: f64, hi: f64) -> UvBox {
        let b = self.curve.bounds(lo, hi);
        UvBox::new(place(b.low()), place(b.high()))
    }

    /// The nearest point by the search every curve shares, over the curve's window for the line through `uv` square to
    /// the plane, within the distance in the plane of the point at the parameter in the range nearest 0.
    fn closest(&self, uv: UvPoint, _seed: Option<f64>, _scratch: &mut Scratch) -> Result<UvClosest, Report> {
        let start = self.curve.range().clamp(0.0);
        let gap = |at: UvPoint| {
            let d = at - uv;
            (d.u() * d.u() + d.v() * d.v()).sqrt()
        };
        let square = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        let window = holding(self.curve.within(lifted(uv), Some(square), gap(self.eval_uv(start))), start);
        let jet = |t: f64| {
            let (point, first, second) = self.curve.eval_second(t);
            [flat(point.to_vec3()), flat(first.rate()), flat(second.rate())]
        };
        let at = nearest_by(jet, lifted(uv).to_vec3(), window);
        let foot = self.eval_uv(at);
        Ok(UvClosest { at, uv: foot, distance: gap(foot) })
    }

    fn family(&self) -> FamilyName {
        self.curve.family()
    }

    fn content(&self, out: &mut Content) {
        self.curve.content(out);
    }

    fn shape(&self) -> Shape {
        self.curve.shape()
    }

    fn fit(&self) -> Option<f64> {
        self.curve.fit().map(occam_units::Length::millimetres)
    }

    /// The curve's B-spline, its poles' shadows on the plane.
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        let form = self.curve.to_bspline(lo, hi)?;
        let poles = form.poles.iter().map(|p| Point::from_vec3(flat(p.to_vec3()))).collect();
        Some(BsplineCurve { poles, ..form })
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use occam_math::Lanes;
    use occam_units::{Angle, Length, Local, Placement, Vector};

    use super::*;
    use crate::conformance::{self, Check, WrongSecondCurve};
    use crate::param_curve::ParamCurve;
    use crate::{Circle, Ellipse, Hyperbola, Line, OffsetCurve, Parabola, SplineCurve, TrimmedCurve};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    /// The plane's frame at `(x, y)`, turned by `angle` about its normal: one that draws in the plane `z = 0`.
    fn flat_frame(x: f64, y: f64, angle: f64) -> Placement<Local, World> {
        let turn = Placement::<Local, Local>::rotation(
            Direction::normalised(Vec3::new(0.0, 0.0, 1.0)),
            Angle::from_radians(angle),
        );
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(x, y, 0.0)));
        turn.then(&Placement::identity()).then(&shift)
    }

    fn at(x: f64, y: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, 0.0))
    }

    fn circle() -> Circle {
        Circle::placed(&flat_frame(3.0, 1.0, 0.4), mm(2.0)).unwrap()
    }

    fn ellipse() -> Ellipse {
        Ellipse::placed(&flat_frame(-1.0, 2.0, 1.1), mm(2.5), mm(1.0)).unwrap()
    }

    fn spline(rational: bool) -> SplineCurve {
        let poles = vec![at(0.0, 0.0), at(1.0, 2.0), at(2.5, 1.5), at(3.0, -0.5), at(4.5, 0.5)];
        let weights = rational.then(|| vec![1.0, 0.5, 2.0, 1.25, 1.0]);
        let knots = vec![0.0, 0.0, 0.0, 0.4, 0.7, 1.0, 1.0, 1.0];
        SplineCurve::new(&BsplineCurve { degree: 2, knots, poles, weights }).unwrap()
    }

    /// The suite over `curve` read in its plane, from `lo` to `hi`.
    fn passes<C: ParametricCurve>(curve: C, (lo, hi): (f64, f64)) {
        let family = curve.family();
        let c = UvCurve::new(curve).unwrap_or_else(|report| panic!("{family:?} is refused: {report:?}"));
        let failures = conformance::param_curve(&c, lo, hi);
        assert!(failures.is_empty(), "{family:?} from {lo} to {hi}: {failures:#?}");
    }

    #[test]
    fn every_kind_in_parameter_space_passes_the_curve_suite() {
        let line = Line::new(at(1.0, 2.0), Direction::normalised(Vec3::new(0.6, 0.8, 0.0)));
        passes(line, (-2.0, 3.0));
        passes(line.trimmed(-1.0, 4.0).unwrap(), (-1.0, 4.0));
        passes(circle(), (0.0, TAU));
        passes(circle().trimmed(5.0, 7.0).unwrap(), (5.0, 7.0));
        passes(ellipse(), (-1.0, 4.0));
        passes(Hyperbola::placed(&flat_frame(0.5, 0.5, -0.3), mm(1.0), mm(1.5)).unwrap(), (-1.5, 1.2));
        passes(Parabola::placed(&flat_frame(2.0, -1.0, 0.7), mm(0.8)).unwrap(), (-3.0, 2.0));
        passes(spline(false), (0.0, 1.0));
        passes(spline(true), (0.1, 0.9));
        passes(
            SplineCurve::bezier(vec![at(0.0, 0.0), at(1.0, 3.0), at(3.0, 2.0), at(4.0, 0.0)], None).unwrap(),
            (0.0, 1.0),
        );
        let square = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        passes(OffsetCurve::new(circle(), mm(0.5), square).unwrap(), (0.3, 2.0));
        passes(TrimmedCurve::new(ellipse(), 0.5, 2.5).unwrap(), (0.5, 2.5));
    }

    #[test]
    fn the_suite_catches_a_parameter_curve_with_a_wrong_second_derivative() {
        let c = UvCurve::new(WrongSecondCurve(circle())).unwrap();
        assert_eq!(conformance::checks(&conformance::param_curve(&c, 0.0, TAU)), [Check::Differences]);
    }

    /// What refused `curve`: its code and the height it names.
    fn refused<C: ParametricCurve>(curve: C) -> (occam_report::Code, Option<f64>) {
        let report = UvCurve::new(curve).map(|_| ()).unwrap_err();
        (report.code, report.value("height").and_then(occam_report::NamedValue::number))
    }

    #[test]
    fn a_curve_that_leaves_the_plane_is_refused_with_how_far() {
        // A circle of radius 2 tilted half a radian about x: it reaches 2 sin 0.5 above and below the plane.
        let tilt = Placement::<Local, Local>::rotation(
            Direction::normalised(Vec3::new(1.0, 0.0, 0.0)),
            Angle::from_radians(0.5),
        );
        let (code, height) = refused(Circle::placed(&tilt.then(&Placement::identity()), mm(2.0)).unwrap());
        assert_eq!(code, occam_report::Code::NotInPlane);
        assert!(height.is_some_and(|h| (h - 2.0 * occam_math::sin(0.5)).abs() < 1e-9), "{height:?}");
        // A line without end half a unit above the plane: a stretch of it decides.
        let lifted =
            Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 0.5)), Direction::normalised(Vec3::new(0.6, 0.8, 0.0)));
        let (code, height) = refused(lifted);
        assert_eq!(code, occam_report::Code::NotInPlane);
        assert!(height.is_some_and(|h| (h - 0.5).abs() < 1e-9), "{height:?}");
        assert!(refused(lifted.trimmed(10.0, 20.0).unwrap()).1.is_some(), "a segment of it, far from 0");
    }

    #[test]
    fn a_curve_off_the_plane_only_between_samples_is_refused_as_its_box_holds_it() {
        // One pole of the spline lifted a millionth: the curve leaves the plane by less than that, near its middle
        // alone, and its box, which holds the poles, shows it.
        let mut poles = vec![at(0.0, 0.0), at(1.0, 2.0), at(2.5, 1.5), at(3.0, -0.5), at(4.5, 0.5)];
        poles[2] = Point::from_vec3(Vec3::new(2.5, 1.5, 1e-6));
        let knots = vec![0.0, 0.0, 0.0, 0.4, 0.7, 1.0, 1.0, 1.0];
        let lifted = SplineCurve::new(&BsplineCurve { degree: 2, knots, poles, weights: None }).unwrap();
        let (code, height) = refused(lifted);
        assert_eq!(code, occam_report::Code::NotInPlane);
        assert!(height.is_some_and(|h| (1e-6..1.1e-6).contains(&h)), "{height:?}");
    }

    #[test]
    fn lanes_give_the_bits_of_single_calls() {
        let c = UvCurve::new(spline(true)).unwrap();
        let ts = [0.1, 0.39, 0.4, 0.95];
        let (uv, d1, d2) = c.eval_second(Lanes::new(ts));
        for (lane, t) in ts.into_iter().enumerate() {
            let (want, w1, w2) = c.second(t);
            let got = [uv.u(), uv.v(), d1.u(), d1.v(), d2.u(), d2.v()].map(|x| x.to_array()[lane].to_bits());
            assert_eq!(got, [want.u(), want.v(), w1.u(), w1.v(), w2.u(), w2.v()].map(f64::to_bits), "{t}");
        }
    }

    #[test]
    fn its_b_spline_shape_content_and_fit_are_its_curves_in_the_plane() {
        let c: &dyn ParamCurve = &UvCurve::new(circle().trimmed(0.5, 2.0).unwrap()).unwrap();
        let form = c.to_bspline(0.5, 2.0).unwrap();
        assert!(form.poles.iter().all(|p| p.to_vec3().z.to_bits() == 0), "{form:?}");
        assert_eq!(form.knots.first(), Some(&0.5));
        assert_eq!((c.family(), c.shape().form, c.fit()), (FamilyName("circle"), "circle", None));
        let mut content = Content::default();
        c.content(&mut content);
        let mut base = Content::default();
        crate::Curve::content(&circle().trimmed(0.5, 2.0).unwrap(), &mut base);
        assert_eq!(content, base);
        let mut out = vec![];
        c.derivatives(1.0, 3, &mut out, &mut Scratch::default()).unwrap();
        assert_eq!(out.len(), 3);
        let b = c.bounds(0.5, 2.0);
        assert!(b.lo.u() <= c.uv(1.0).u() && c.uv(1.0).v() <= b.hi.v());
    }

    fn unit_circle() -> UvCurve<Circle> {
        UvCurve::new(Circle::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap()).unwrap()
    }

    #[test]
    fn the_nearest_point_is_found_from_far_off_whichever_way() {
        // From (19, 21), the unit circle's start at (1, 0) is 27.66 away and its nearest point 27.32: the search must
        // reach that far in the plane, in both coordinates.
        let near =
            ParamCurve::closest(&unit_circle(), UvPoint::new(19.0, 21.0), None, &mut Scratch::default()).unwrap();
        let want = (19.0_f64 * 19.0 + 21.0 * 21.0).sqrt() - 1.0;
        assert!((near.distance - want).abs() < 1e-12 && (near.at - occam_math::atan2(21.0, 19.0)).abs() < 1e-12);
    }

    #[test]
    fn a_curve_in_the_plane_passes_its_fit_on_and_one_through_it_is_refused() {
        let x = Direction::normalised(Vec3::new(1.0, 0.0, 0.0));
        let fitted =
            UvCurve::new(conformance::Fitted(Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), x))).unwrap();
        assert_eq!(ParamCurve::fit(&fitted), Some(0.25));
        // A line along z through the plane at its parameter 1, a hundred out: off the plane everywhere else, however
        // little of it around that one point is looked at.
        let z = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        let up = Line::new(Point::from_vec3(Vec3::new(100.0, 0.0, -1.0)), z);
        assert_eq!(UvCurve::new(up).map(|_| ()).unwrap_err().code, Code::NotInPlane);
    }
}
