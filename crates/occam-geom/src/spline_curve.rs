//! Source: piegl-tiller-1997
//! The B-spline curve, rational or not, and the Bézier curve, which is one with a single span (PARITY 2.1):
//! `C(t) = sum N(i, p)(t) w(i) P(i) / sum N(i, p)(t) w(i)`. Its derivatives are the basis's (A3.2), divided by Leibniz's
//! rule where it is rational (A4.2). Lanes on one span share its knots and poles; lanes across spans are evaluated one
//! at a time, so that each has the bits of a single call. Its foot is searched for, as every curve's without a formula.

use occam_math::Real;
use occam_report::{NamedValue, Quantity, Report};
use occam_units::{Bounds, ParamKind, Point, Tangent, Vec3, World};

use crate::basis::{basis, rows_len, span};
use crate::batch::Scratch;
use crate::bspline::{BsplineCurve, MAX_DEGREE, bezier_knots, cartesian, checked_knots, checked_poles, homogeneous};
use crate::curve::ParametricCurve;
use crate::domain::{ParamRange, Wrap};
use crate::identity::{Content, FamilyName, Shape};
use crate::knots::{Rows, canonical};
use crate::lanewise::{MAX_LANES, from_lanes, lanes};
use crate::spline::{CAPACITIES, curve_quotient};
use crate::support::{everything, padded};

/// A B-spline or Bézier curve, rational or not.
#[derive(Clone, Debug, PartialEq)]
pub struct SplineCurve {
    degree: usize,
    knots: Vec<f64>,
    /// The homogeneous poles, `(w x, w y, w z, w)`.
    poles: Vec<[f64; 4]>,
    rational: bool,
    bezier: bool,
    range: ParamRange,
}

/// A count as a number.
fn number(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

impl SplineCurve {
    /// The curve `form` describes.
    ///
    /// # Errors
    ///
    /// `geom.out-of-range` for a degree outside 1 to 25, `geom.wrong-count` for too few poles or the wrong number of
    /// knots or weights, `geom.knots-not-valid` for knots that decrease or repeat too often, `units.not-finite` for a
    /// knot or pole that is not finite, and `geom.weight-not-positive` for a weight that is not above zero.
    pub fn new(form: &BsplineCurve) -> Result<Self, Report> {
        let (p, n) = checked_knots(form.degree, form.poles.len(), &form.knots)?;
        checked_poles(&form.poles, form.weights.as_deref())?;
        let knots = canonical(&form.knots);
        let at = |i: usize| knots.get(i).copied().unwrap_or(f64::NAN);
        let range = ParamRange { lo: at(p), hi: at(n), wrap: Wrap::Open, kind: ParamKind::Knot };
        let poles = homogeneous(&form.poles, form.weights.as_deref());
        Ok(Self { degree: p, knots, poles, rational: form.weights.is_some(), bezier: false, range })
    }

    /// The Bézier curve of these poles and their weights, if it is rational: degree one less than the poles, from 0
    /// to 1.
    ///
    /// # Errors
    ///
    /// As [`SplineCurve::new`]: a degree outside 1 to 25 is a count outside 2 to 26.
    pub fn bezier(poles: Vec<Point<World>>, weights: Option<Vec<f64>>) -> Result<Self, Report> {
        let degree = u16::try_from(poles.len().saturating_sub(1)).unwrap_or(u16::MAX);
        let knots = bezier_knots(usize::from(degree.min(MAX_DEGREE)));
        Ok(Self { bezier: true, ..Self::new(&BsplineCurve { degree, knots, poles, weights })? })
    }

    /// The span holding `t`.
    fn span(&self, t: f64) -> usize {
        span(&self.knots, self.degree, self.poles.len(), t)
    }

    /// How many numbers an evaluation to `order` works in: the basis's rows and derivatives, then the weight's.
    fn layout(&self, order: usize) -> [usize; 3] {
        let orders = order.saturating_add(1);
        [rows_len(self.degree, order), orders.saturating_mul(self.degree.saturating_add(1)), orders]
    }

    /// The point and its derivatives to `order` at `t`, which lies on span `s`, into `out`, lowest first: the sums of
    /// the basis's derivatives times the poles, then, where it is rational, their quotient. `work` holds
    /// [`SplineCurve::layout`]'s numbers.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn jets<R: Real>(&self, on: usize, t: R, order: usize, work: &mut [R], out: &mut [Vec3<R>]) {
        let [rows, ders, _] = self.layout(order);
        let Some((rows, rest)) = work.split_at_mut_checked(rows) else { return };
        let Some((ders, weights)) = rest.split_at_mut_checked(ders) else { return };
        let degree = self.degree;
        basis(&self.knots, (degree, on), t, order, rows, ders);
        let near = self.poles.get(on.saturating_sub(degree)..=on).unwrap_or_default();
        let zero = R::splat(0.0);
        let rates = ders.chunks_exact(degree.saturating_add(1)).zip(out.iter_mut()).zip(weights.iter_mut());
        for ((along, slot), weight) in rates.take(order.saturating_add(1)) {
            let mut sums = [zero; 4];
            for (function, pole) in along.iter().zip(near) {
                for (sum, part) in sums.iter_mut().zip(pole) {
                    *sum = *sum + *function * R::splat(*part);
                }
            }
            let [sum_x, sum_y, sum_z, sum_w] = sums;
            (*slot, *weight) = (Vec3::new(sum_x, sum_y, sum_z), sum_w);
        }
        if self.rational {
            curve_quotient(order, out, weights);
        }
    }

    /// [`SplineCurve::jets`] to `order` on the stack, with room for `M` functions of the basis.
    #[inline(always)]
    fn within<R: Real, const N: usize, const M: usize>(&self, s: usize, t: R, order: usize) -> [Vec3<R>; N] {
        let zero = R::splat(0.0);
        let mut work = [[zero; M]; 6];
        let mut out = [Vec3::new(zero, zero, zero); N];
        self.jets(s, t, order, work.as_flattened_mut(), &mut out);
        out
    }

    /// [`SplineCurve::within`], with the least room the degree needs.
    #[inline(always)]
    fn at<R: Real, const N: usize>(&self, s: usize, t: R, order: usize) -> [Vec3<R>; N] {
        let [small, middle, _] = CAPACITIES;
        if self.degree < small {
            self.within::<R, N, 4>(s, t, order)
        } else if self.degree < middle {
            self.within::<R, N, 8>(s, t, order)
        } else {
            self.within::<R, N, 26>(s, t, order)
        }
    }

    /// The point and its derivatives to `order`, `N` of them: in lanes where every lane is on one span, and otherwise a
    /// lane at a time.
    #[inline(always)]
    fn rungs<R: Real, const N: usize>(&self, t: R, order: usize) -> [Vec3<R>; N] {
        const { assert!(R::LANES <= MAX_LANES, "more lanes than a lane-at-a-time evaluation holds") };
        let ts = lanes(t);
        let first = self.span(ts.first().copied().unwrap_or(f64::NAN));
        if ts.iter().take(R::LANES).all(|t| self.span(*t) == first) {
            return self.at(first, t, order);
        }
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let each: [[Vec3; N]; MAX_LANES] = std::array::from_fn(|k| match ts.get(k) {
            Some(&t) if k < R::LANES => self.at::<f64, N>(self.span(t), t, order),
            _ => [zero; N],
        });
        std::array::from_fn(|slot| from_lanes(|k| each.get(k).and_then(|e| e.get(slot)).copied().unwrap_or(zero)))
    }
}

impl ParametricCurve for SplineCurve {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        let [point] = self.rungs::<R, 1>(t, 0);
        Point::from_vec3(point)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        let [point, rate] = self.rungs::<R, 2>(t, 1);
        (Point::from_vec3(point), Tangent::new(rate))
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let order = usize::from(order);
        let len = self.layout(order).into_iter().fold(0, usize::saturating_add);
        let (work, jets) = scratch.reals_and_vectors(len, order.saturating_add(1));
        self.jets(self.span(t), t, order, work, jets);
        out.extend(jets.iter().skip(1).map(|v| Tangent::new(*v)));
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    /// The box of the poles of the spans from `lo` to `hi`, which hold the curve there where every weight is positive,
    /// padded for rounding. A part outside the range gives all of space.
    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let (lo, hi) = if hi < lo { (hi, lo) } else { (lo, hi) };
        if !(self.range.contains(lo) && self.range.contains(hi)) {
            return everything();
        }
        let near = self.poles.get(self.span(lo).saturating_sub(self.degree)..=self.span(hi)).unwrap_or_default();
        let points = near.iter().map(|[x, y, z, w]| Point::from_vec3(Vec3::new(x / w, y / w, z / w)));
        Bounds::of(points).map_or_else(everything, |hull| padded(hull, number(self.degree.saturating_add(2))))
    }

    fn family(&self) -> FamilyName {
        FamilyName("bspline")
    }

    fn content(&self, out: &mut Content) {
        for x in [u8::from(self.bezier), u8::from(self.rational)] {
            out.push(f64::from(x));
        }
        out.push(number(self.degree));
        for x in self.knots.iter().chain(self.poles.iter().flatten()) {
            out.push(*x);
        }
    }

    fn shape(&self) -> Shape {
        let count = |name: &str, n: usize| NamedValue::new(name, Quantity::Count, number(n));
        let values = vec![
            count("degree", self.degree),
            count("poles", self.poles.len()),
            count("rational", usize::from(self.rational)),
        ];
        Shape { form: if self.bezier { "bezier" } else { "bspline" }, values }
    }

    /// Itself from `lo` to `hi`, cut by knot insertion.
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        let part = self.range.trimmed(lo, hi).ok()?;
        let whole = Rows { knots: self.knots.clone(), degree: self.degree, poles: vec![self.poles.clone()] };
        let cut = whole.restricted(part.lo, part.hi);
        let (poles, weights) = cartesian(&cut.poles.concat(), self.rational);
        let degree = u16::try_from(self.degree).unwrap_or(u16::MAX);
        Some(BsplineCurve { degree, knots: cut.knots, poles, weights })
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Lanes;
    use occam_report::Code;

    use super::*;
    use crate::conformance::{self, Check, WrongSecondCurve, tolerance};
    use crate::curve::Curve;

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    /// `n` poles along a twisting path, with nothing symmetric.
    fn poles(n: u32) -> Vec<Point<World>> {
        (0..n)
            .map(f64::from)
            .map(|x| point(10.0 + 2.0 * x, -5.0 + ((x * 7.0) % 5.0) - 0.3 * x, 3.0 + ((x * 3.0) % 4.0)))
            .collect()
    }

    fn curve(degree: u16, knots: Vec<f64>, rational: bool) -> SplineCurve {
        let n = u32::try_from(knots.len().saturating_sub(usize::from(degree) + 1)).unwrap();
        let poles = poles(n);
        let weights = rational.then(|| [1.0, 0.5, 2.0, 1.25, 0.75].into_iter().cycle().take(poles.len()).collect());
        SplineCurve::new(&BsplineCurve { degree, knots, poles, weights }).unwrap()
    }

    /// Curves that between them have each degree's room, a rational form, a repeated knot inside and unclamped ends.
    fn curves() -> Vec<SplineCurve> {
        vec![
            SplineCurve::bezier(poles(4), None).unwrap(),
            curve(2, vec![0.0, 0.0, 0.0, 0.4, 0.4, 1.0, 1.0, 1.0], true),
            curve(5, [-1.0; 6].into_iter().chain([0.3, 0.6, 0.6]).chain([2.0; 6]).collect(), false),
            curve(11, [0.0; 12].into_iter().chain([1.0; 12]).collect(), true),
            curve(3, (0..10).map(f64::from).collect(), false),
        ]
    }

    #[test]
    fn the_spline_curves_pass_the_conformance_suite() {
        for c in curves() {
            let r = c.range;
            for (lo, hi) in [(r.lo, r.hi), (r.lo + 0.2 * (r.hi - r.lo), r.lo + 0.65 * (r.hi - r.lo))] {
                let failures = conformance::curve(&c, lo, hi);
                assert!(failures.is_empty(), "degree {} from {lo} to {hi}: {failures:#?}", c.degree);
            }
        }
    }

    #[test]
    fn the_suite_catches_a_spline_curve_with_a_wrong_second_derivative() {
        let c = curves().swap_remove(1);
        let failed = conformance::checks(&conformance::curve(&WrongSecondCurve(c), 0.0, 1.0));
        assert_eq!(failed, [Check::Differences]);
    }

    #[test]
    fn a_polynomial_bezier_has_the_derivatives_of_its_formula() {
        // (t, t^2, t^3) as a cubic: its poles are (0, 0, 0), (1/3, 0, 0), (2/3, 1/3, 0) and (1, 1, 1).
        let third = 1.0 / 3.0;
        let poles =
            vec![point(0.0, 0.0, 0.0), point(third, 0.0, 0.0), point(2.0 * third, third, 0.0), point(1.0, 1.0, 1.0)];
        let c = SplineCurve::bezier(poles, None).unwrap();
        let t = 0.4;
        let mut out = vec![];
        Curve::derivatives(&c, t, 5, &mut out, &mut Scratch::default()).unwrap();
        let want = [
            Vec3::new(1.0, 2.0 * t, 3.0 * t * t),
            Vec3::new(0.0, 2.0, 6.0 * t),
            Vec3::new(0.0, 0.0, 6.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        ];
        assert_eq!(out.len(), 5);
        for (got, want) in out.iter().zip(want) {
            assert!((got.rate() - want).norm() < 1e-14, "{got:?} against {want:?}");
        }
        assert!((c.point(t).to_vec3() - Vec3::new(t, t * t, t * t * t)).norm() < 1e-15);
        assert_eq!((Curve::shape(&c).form, Curve::range(&c).lo, Curve::range(&c).hi), ("bezier", 0.0, 1.0));
    }

    #[test]
    fn a_rational_quadratic_is_a_circle_exactly() {
        let w = 0.5f64.sqrt();
        let c = SplineCurve::bezier(
            vec![point(3.0, 0.0, 1.0), point(3.0, 3.0, 1.0), point(0.0, 3.0, 1.0)],
            Some(vec![1.0, w, 1.0]),
        )
        .unwrap();
        for i in 0..=20 {
            let p = c.point(f64::from(i) / 20.0).to_vec3();
            assert!(((p.x * p.x + p.y * p.y).sqrt() - 3.0).abs() < 1e-15, "{p:?}");
            assert_eq!(p.z, 1.0);
        }
    }

    #[test]
    fn lanes_across_a_knot_give_the_bits_of_single_calls() {
        let c = curves().swap_remove(2);
        for ts in [[0.25, 0.29, 0.3, 0.5], [0.61, 0.7, 1.0, 1.99]] {
            let (p, d) = c.eval_first(Lanes::new(ts));
            for (lane, t) in ts.into_iter().enumerate() {
                let (want_p, want_d) = c.first(t);
                let lanes = |v: Vec3<Lanes<4>>| [v.x, v.y, v.z].map(|x| x.to_array()[lane].to_bits());
                let bits = |v: Vec3| [v.x, v.y, v.z].map(f64::to_bits);
                assert_eq!((lanes(p.to_vec3()), lanes(d.rate())), (bits(want_p.to_vec3()), bits(want_d.rate())), "{t}");
            }
        }
    }

    #[test]
    fn a_part_keeps_the_points_and_parameters_it_had() {
        for c in curves() {
            let r = c.range;
            let (lo, hi) = (r.lo + 0.15 * (r.hi - r.lo), r.lo + 0.8 * (r.hi - r.lo));
            let part = SplineCurve::new(&Curve::to_bspline(&c, lo, hi).unwrap()).unwrap();
            assert_eq!((part.range.lo, part.range.hi), (lo, hi));
            for f in [0.0, 0.3, 0.5, 0.9, 1.0] {
                let t = lo + (hi - lo) * f;
                assert!((part.point(t).to_vec3() - c.point(t).to_vec3()).norm() < 1e-12, "degree {} at {t}", c.degree);
            }
            assert_eq!(Curve::to_bspline(&c, r.lo, r.hi + 1.0), None);
            assert_eq!(Curve::to_bspline(&c, hi, lo), None, "back to front");
        }
    }

    #[test]
    fn a_box_outside_the_range_is_all_of_space_and_a_point_inside_is_held() {
        let c = curves().swap_remove(1);
        assert!(Curve::bounds(&c, -0.5, 0.5).contains(point(1e300, -1e300, 1e300)));
        let b = Curve::bounds(&c, 0.5, 0.5);
        assert!(b.contains(c.point(0.5)) && !b.contains(point(1e300, 0.0, 0.0)));
    }

    #[test]
    fn a_rational_box_is_its_poles_padded_and_a_part_back_to_front_is_the_same() {
        // Each coordinate's extremes move if a pole is read without its weight: no two mistakes cancel.
        let poles = vec![point(0.0, 1.0, 2.0), point(3.0, 1.5, 2.0), point(2.0, 4.0, 5.0)];
        let c = SplineCurve::bezier(poles, Some(vec![1.0, 2.0, 0.5])).unwrap();
        let b = Curve::bounds(&c, 0.0, 1.0);
        let margin = 64.0 * 4.0 * f64::EPSILON * 5.0;
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        for (got, want) in [(low.x, 0.0), (low.y, 1.0), (low.z, 2.0), (high.x, 3.0), (high.y, 4.0), (high.z, 5.0)] {
            let rounding = 4.0 * f64::EPSILON * want.abs();
            assert!(((got - want).abs() - margin).abs() <= 1e-3 * margin + rounding, "{got} against {want}: {b:?}");
        }
        let many = curves().swap_remove(1);
        assert_eq!(Curve::bounds(&many, 0.9, 0.1), Curve::bounds(&many, 0.1, 0.9), "back to front, across a knot");
    }

    #[test]
    fn a_curve_with_the_wrong_numbers_is_refused() {
        let good = BsplineCurve {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            poles: poles(3),
            weights: Some(vec![1.0; 3]),
        };
        let code = |f: BsplineCurve| SplineCurve::new(&f).unwrap_err().code;
        assert_eq!(code(BsplineCurve { poles: poles(2), ..good.clone() }), Code::WrongCount);
        assert_eq!(
            code(BsplineCurve { knots: vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0], ..good.clone() }),
            Code::KnotsNotValid
        );
        assert_eq!(code(BsplineCurve { weights: Some(vec![1.0, 0.0, 1.0]), ..good.clone() }), Code::WeightNotPositive);
        assert_eq!(code(BsplineCurve { degree: 30, ..good.clone() }), Code::OutOfRange);
        assert!(SplineCurve::new(&good).is_ok());
        assert_eq!(SplineCurve::bezier(poles(1), None).unwrap_err().code, Code::OutOfRange);
        assert_eq!(SplineCurve::bezier(poles(30), None).unwrap_err().code, Code::OutOfRange);
    }

    #[test]
    fn its_shape_and_content_name_what_defines_it() {
        let c = curves().swap_remove(3);
        let shape = Curve::shape(&c);
        let numbers: Vec<Option<f64>> = shape.values.iter().map(NamedValue::number).collect();
        assert_eq!((shape.form, numbers), ("bspline", vec![Some(11.0), Some(12.0), Some(1.0)]));
        let mut content = Content::default();
        Curve::content(&c, &mut content);
        assert_eq!(content.words().len(), 1 + 7 + 2 + 1 + 24 + 12 * 4);
        assert_eq!((Curve::family(&c), Curve::fit(&c)), (FamilyName("bspline"), None));
        let tol = tolerance();
        let far = Curve::closest(&c, point(0.0, 0.0, 0.0), None, tol, &mut Scratch::default()).unwrap();
        assert!(c.range.contains(far.at));
    }
}
