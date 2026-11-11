//! Source: original
//! Families with a seeded fault, which the suite must catch: each passes everything through to a real family except a
//! second derivative wrong by a thousandth.

use occam_math::Real;
use occam_report::Report;
use occam_units::{Bounds, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::support::splat;
use crate::{
    Analytic, Closest, Content, Degeneracy, Domain, FamilyName, Jet1, Jet2, ParamRange, Parametric, ParametricCurve,
    Scratch, Shape, UvBox,
};

/// The seeded error.
const SEED: Vec3 = Vec3::new(1e-3, 0.0, 0.0);

/// A surface whose second derivative in `u`, from `eval_second` alone, is wrong.
pub(crate) struct WrongSecond<S>(pub S);

impl<S: Parametric> Parametric for WrongSecond<S> {
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.0.eval_point(uv)
    }

    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        self.0.eval_first(uv)
    }

    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let jet = self.0.eval_second(uv);
        Jet2 { duu: Tangent::new(jet.duu.rate() + splat(SEED)), ..jet }
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        Parametric::derivatives(&self.0, uv, order, out, scratch)
    }

    fn domain(&self) -> Domain {
        Parametric::domain(&self.0)
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        Parametric::bounds(&self.0, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        Parametric::invert(&self.0, p, seed, tol, scratch)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report> {
        Parametric::closest(&self.0, p, seed, tol, scratch)
    }

    fn family(&self) -> FamilyName {
        Parametric::family(&self.0)
    }

    fn content(&self, out: &mut Content) {
        Parametric::content(&self.0, out);
    }

    fn shape(&self) -> Shape {
        Parametric::shape(&self.0)
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        Parametric::degeneracies(&self.0)
    }

    fn analytic(&self, tol: Tolerance) -> Option<Analytic> {
        Parametric::analytic(&self.0, tol)
    }
}

/// A curve whose second derivative is wrong.
pub(crate) struct WrongSecondCurve<C>(pub C);

impl<C: ParametricCurve> ParametricCurve for WrongSecondCurve<C> {
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        self.0.eval_point(t)
    }

    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        self.0.eval_first(t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let start = out.len();
        ParametricCurve::derivatives(&self.0, t, order, out, scratch)?;
        if let Some(second) = out.iter_mut().skip(start).nth(1) {
            *second = Tangent::new(second.rate() + SEED);
        }
        Ok(())
    }

    fn range(&self) -> ParamRange {
        ParametricCurve::range(&self.0)
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        ParametricCurve::bounds(&self.0, lo, hi)
    }

    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance, scratch: &mut Scratch) -> Result<f64, Report> {
        ParametricCurve::invert(&self.0, p, seed, tol, scratch)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        ParametricCurve::closest(&self.0, p, seed, tol, scratch)
    }

    fn family(&self) -> FamilyName {
        ParametricCurve::family(&self.0)
    }

    fn content(&self, out: &mut Content) {
        ParametricCurve::content(&self.0, out);
    }

    fn shape(&self) -> Shape {
        ParametricCurve::shape(&self.0)
    }
}
