//! Source: original
//! The parameter-curve seam: a curve in a face's parameters, in the two layers every family has (S1 item 7: without one,
//! a boolean's phases took 3.45 times as long; PARITY 2.3). [`ParametricParamCurve`] is the family layer, generic over
//! [`Real`]; [`ParamCurve`] is what everything else calls, joined to it by one blanket impl that checks each call.
//!
//! Its coordinates are the surface's parameters, which may be lengths, angles or knots, and are plain numbers here, as
//! [`UvPoint`] keeps them: a distance between two of them is in the parameters' own measure, with no unit. Nothing in
//! this layer is a point in `World`.

use occam_math::Real;
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{UvPoint, UvVector};

use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::domain::{ParamRange, UvBox};
use crate::identity::{Content, FamilyName, Shape};

/// The point of a parameter curve nearest a point of the face's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvClosest {
    /// Its parameter on the curve.
    pub at: f64,
    /// The point, in the face's parameters.
    pub uv: UvPoint,
    /// How far it is, in the parameters' own measure.
    pub distance: f64,
}

/// The first layer, which a parameter-curve family implements, generic over [`Real`]. As for curves in space, the
/// `eval_*` methods are total and give NaN where the curve is undefined.
pub trait ParametricParamCurve: Sized + Send + Sync + 'static {
    /// The point.
    fn eval_uv<R: Real>(&self, t: R) -> UvPoint<R>;

    /// The point and its derivative.
    fn eval_first<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>);

    /// The point and its first and second derivatives.
    fn eval_second<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>, UvVector<R>);

    /// Appends the derivatives of order 1 to `order` at `t`, which is inside the range, lowest first.
    ///
    /// # Errors
    ///
    /// Where the family has no derivative of an order asked for.
    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<UvVector>, scratch: &mut Scratch) -> Result<(), Report>;

    /// Where it is defined.
    fn range(&self) -> ParamRange;

    /// A box in the face's parameters holding the curve from `lo` to `hi`, rounding included.
    fn bounds(&self, lo: f64, hi: f64) -> UvBox;

    /// The point of the curve, within its range, nearest `uv`, which is finite.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn closest(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<UvClosest, Report>;

    /// The parameter of the point nearest `uv`, which is finite: [`ParametricParamCurve::closest`]'s, unless the
    /// family has a cheaper way to it.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn invert(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<f64, Report> {
        Ok(self.closest(uv, seed, scratch)?.at)
    }

    /// The family's name.
    fn family(&self) -> FamilyName;

    /// Adds the numbers that define it to `out`, which already holds the family's name.
    fn content(&self, out: &mut Content);

    /// The form and parameters a description shows.
    fn shape(&self) -> Shape;

    /// How closely a sampled parameter curve fits what it was sampled from, in the face's parameters. `None` for one
    /// that is exact.
    fn fit(&self) -> Option<f64> {
        None
    }

    /// The rational B-spline it is exactly from `lo` to `hi`, its poles in the face's parameters as `(u, v, 0)`, if it
    /// has one there.
    fn to_bspline(&self, _lo: f64, _hi: f64) -> Option<BsplineCurve> {
        None
    }
}

/// The second layer, which everything outside a family calls: dyn-compatible, and checking each call as [`crate::Curve`]
/// does.
pub trait ParamCurve: Send + Sync {
    /// As [`ParametricParamCurve::eval_uv`].
    fn uv(&self, t: f64) -> UvPoint;

    /// As [`ParametricParamCurve::eval_first`].
    fn first(&self, t: f64) -> (UvPoint, UvVector);

    /// As [`ParametricParamCurve::eval_second`].
    fn second(&self, t: f64) -> (UvPoint, UvVector, UvVector);

    /// Replaces `out` with the derivatives of order 1 to `order`, lowest first.
    ///
    /// # Errors
    ///
    /// `geom.undefined-parameter` outside the range, and what [`ParametricParamCurve::derivatives`] reports.
    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<UvVector>, scratch: &mut Scratch) -> Result<(), Report>;

    /// As [`ParametricParamCurve::closest`].
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point that is not finite, and what [`ParametricParamCurve::closest`] reports.
    fn closest(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<UvClosest, Report>;

    /// As [`ParametricParamCurve::invert`].
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point that is not finite, and what [`ParametricParamCurve::invert`] reports.
    fn invert(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<f64, Report>;

    /// As [`ParametricParamCurve::range`].
    fn range(&self) -> ParamRange;

    /// As [`ParametricParamCurve::bounds`].
    fn bounds(&self, lo: f64, hi: f64) -> UvBox;

    /// As [`ParametricParamCurve::family`].
    fn family(&self) -> FamilyName;

    /// Replaces `out` with the family's name and the numbers that define the curve.
    fn content(&self, out: &mut Content);

    /// As [`ParametricParamCurve::shape`].
    fn shape(&self) -> Shape;

    /// As [`ParametricParamCurve::fit`].
    fn fit(&self) -> Option<f64>;

    /// As [`ParametricParamCurve::to_bspline`].
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve>;
}

/// Refuses a point of the face's parameters that is not finite, naming the first part that is not.
fn finite(uv: UvPoint) -> Result<(), Report> {
    match [uv.u(), uv.v()].into_iter().find(|x| !x.is_finite()) {
        None => Ok(()),
        Some(x) => Err(Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::new(
            "value",
            Quantity::Ratio,
            x,
        ))),
    }
}

/// The one place the family trait becomes the object trait.
impl<P: ParametricParamCurve> ParamCurve for P {
    fn uv(&self, t: f64) -> UvPoint {
        self.eval_uv(t)
    }

    fn first(&self, t: f64) -> (UvPoint, UvVector) {
        self.eval_first(t)
    }

    fn second(&self, t: f64) -> (UvPoint, UvVector, UvVector) {
        self.eval_second(t)
    }

    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<UvVector>, scratch: &mut Scratch) -> Result<(), Report> {
        out.clear();
        ParametricParamCurve::range(self).check(t)?;
        ParametricParamCurve::derivatives(self, t, order, out, scratch)
    }

    fn closest(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<UvClosest, Report> {
        finite(uv)?;
        ParametricParamCurve::closest(self, uv, seed, scratch)
    }

    fn invert(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<f64, Report> {
        finite(uv)?;
        ParametricParamCurve::invert(self, uv, seed, scratch)
    }

    fn range(&self) -> ParamRange {
        ParametricParamCurve::range(self)
    }

    fn bounds(&self, lo: f64, hi: f64) -> UvBox {
        ParametricParamCurve::bounds(self, lo, hi)
    }

    fn family(&self) -> FamilyName {
        ParametricParamCurve::family(self)
    }

    fn content(&self, out: &mut Content) {
        out.start(ParametricParamCurve::family(self));
        ParametricParamCurve::content(self, out);
    }

    fn shape(&self) -> Shape {
        ParametricParamCurve::shape(self)
    }

    fn fit(&self) -> Option<f64> {
        ParametricParamCurve::fit(self)
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        ParametricParamCurve::to_bspline(self, lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A family that presents and fits as no exact one does, and has no closed form, to show the object layer passes
    /// each on and checks each call.
    struct Diagonal;

    impl ParametricParamCurve for Diagonal {
        fn eval_uv<R: Real>(&self, t: R) -> UvPoint<R> {
            UvPoint::new(t, t)
        }

        fn eval_first<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>) {
            (UvPoint::new(t, t), UvVector::new(R::splat(1.0), R::splat(1.0)))
        }

        fn eval_second<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>, UvVector<R>) {
            let (uv, d) = self.eval_first(t);
            (uv, d, UvVector::new(R::splat(0.0), R::splat(0.0)))
        }

        fn derivatives(
            &self,
            _t: f64,
            order: u8,
            out: &mut Vec<UvVector>,
            _scratch: &mut Scratch,
        ) -> Result<(), Report> {
            out.extend((1..=order).map(|k| if k == 1 { UvVector::new(1.0, 1.0) } else { UvVector::new(0.0, 0.0) }));
            Ok(())
        }

        fn range(&self) -> ParamRange {
            ParamRange { lo: 0.0, hi: 2.0, wrap: crate::Wrap::Open, kind: occam_units::ParamKind::Knot }
        }

        fn bounds(&self, lo: f64, hi: f64) -> UvBox {
            UvBox::new(UvPoint::new(lo, lo), UvPoint::new(hi, hi))
        }

        fn closest(&self, uv: UvPoint, _seed: Option<f64>, _scratch: &mut Scratch) -> Result<UvClosest, Report> {
            let at = f64::midpoint(uv.u(), uv.v());
            let distance = 0.5_f64.sqrt() * (uv.u() - uv.v()).abs();
            Ok(UvClosest { at, uv: UvPoint::new(at, at), distance })
        }

        fn family(&self) -> FamilyName {
            FamilyName("diagonal")
        }

        fn content(&self, out: &mut Content) {
            out.push(1.0);
        }

        fn shape(&self) -> Shape {
            Shape { form: "diagonal", values: vec![] }
        }

        fn fit(&self) -> Option<f64> {
            Some(0.25)
        }
    }

    /// [`Diagonal`] as an exact family would be: everything passed on but the fit, which it leaves to the default.
    struct Exact;

    impl ParametricParamCurve for Exact {
        fn eval_uv<R: Real>(&self, t: R) -> UvPoint<R> {
            Diagonal.eval_uv(t)
        }

        fn eval_first<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>) {
            Diagonal.eval_first(t)
        }

        fn eval_second<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>, UvVector<R>) {
            Diagonal.eval_second(t)
        }

        fn derivatives(&self, t: f64, order: u8, out: &mut Vec<UvVector>, scratch: &mut Scratch) -> Result<(), Report> {
            ParametricParamCurve::derivatives(&Diagonal, t, order, out, scratch)
        }

        fn range(&self) -> ParamRange {
            ParametricParamCurve::range(&Diagonal)
        }

        fn bounds(&self, lo: f64, hi: f64) -> UvBox {
            ParametricParamCurve::bounds(&Diagonal, lo, hi)
        }

        fn closest(&self, uv: UvPoint, seed: Option<f64>, scratch: &mut Scratch) -> Result<UvClosest, Report> {
            ParametricParamCurve::closest(&Diagonal, uv, seed, scratch)
        }

        fn family(&self) -> FamilyName {
            FamilyName("exact")
        }

        fn content(&self, out: &mut Content) {
            ParametricParamCurve::content(&Diagonal, out);
        }

        fn shape(&self) -> Shape {
            ParametricParamCurve::shape(&Diagonal)
        }
    }

    #[test]
    fn an_exact_family_has_no_fit_to_say() {
        let c: &dyn ParamCurve = &Exact;
        assert_eq!((c.fit(), c.uv(0.5)), (None, UvPoint::new(0.5, 0.5)));
    }

    #[test]
    fn the_object_layer_passes_each_answer_on_and_checks_each_call() {
        let (c, mut scratch): (&dyn ParamCurve, _) = (&Diagonal, Scratch::default());
        assert_eq!(
            (c.uv(0.5), c.first(0.5).1, c.second(0.5).2),
            (UvPoint::new(0.5, 0.5), UvVector::new(1.0, 1.0), UvVector::new(0.0, 0.0))
        );
        let mut out = vec![UvVector::new(9.0, 9.0)];
        c.derivatives(1.0, 2, &mut out, &mut scratch).unwrap();
        assert_eq!(out, [UvVector::new(1.0, 1.0), UvVector::new(0.0, 0.0)]);
        let outside = c.derivatives(3.0, 1, &mut out, &mut scratch).unwrap_err();
        assert_eq!((outside.code, out.len()), (Code::UndefinedParameter, 0), "refused, and the old values gone");
        let near = c.closest(UvPoint::new(1.0, 0.0), None, &mut scratch).unwrap();
        assert_eq!((near.at, c.invert(UvPoint::new(1.0, 0.0), None, &mut scratch).unwrap()), (0.5, 0.5));
        for bad in [UvPoint::new(f64::NAN, 0.0), UvPoint::new(0.0, f64::INFINITY)] {
            assert_eq!(c.closest(bad, None, &mut scratch).unwrap_err().code, Code::NotFinite);
            assert_eq!(c.invert(bad, None, &mut scratch).unwrap_err().code, Code::NotFinite);
        }
        assert_eq!(
            (c.range().hi, c.bounds(0.0, 1.0), c.fit(), c.to_bspline(0.0, 1.0)),
            (2.0, UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(1.0, 1.0)), Some(0.25), None)
        );
        let mut content = Content::default();
        c.content(&mut content);
        assert_eq!(
            (content.words().len(), c.family(), c.shape().form),
            (1 + 8 + 1, FamilyName("diagonal"), "diagonal")
        );
    }
}
