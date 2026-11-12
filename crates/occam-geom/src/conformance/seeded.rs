//! Source: original
//! Families with a seeded fault, which the suite must catch: each passes everything through to a real family except a
//! second derivative wrong by a thousandth, or, for a surface, a first derivative wrong as much or the form it presents.
//! One more passes everything through but its fit: a line that says it was sampled, to show a fit is passed on. A
//! surface may also say it has derivatives to some order alone, to show the limit is passed on.

use occam_math::Real;
use occam_report::Report;
use occam_units::{Bounds, Length, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::line::Line;
use crate::support::splat;
use crate::{
    Analytic, Closest, Content, Degeneracy, Domain, FamilyName, Jet1, Jet2, ParamRange, Parametric, ParametricCurve,
    Scratch, Shape, UvBox,
};

/// The seeded error.
const SEED: Vec3 = Vec3::new(1e-3, 0.0, 0.0);

/// What is wrong with a [`Seeded`] surface.
#[derive(Clone, Copy)]
enum Fault {
    /// Its first derivative in `u`, from `eval_first` and `eval_second` alone, is wrong by [`SEED`].
    First,
    /// Its second derivative in `u`, from `eval_second` alone, is wrong by [`SEED`].
    Second,
    /// It presents this form rather than its own.
    Form(Analytic),
    /// It says it has derivatives to this order alone.
    Capped(u8),
}

/// A surface with a seeded fault.
pub(crate) struct Seeded<S> {
    surface: S,
    fault: Fault,
}

/// `surface` with a first derivative in `u`, from `eval_first` and `eval_second` alone, that is wrong.
pub(crate) const fn wrong_first<S>(surface: S) -> Seeded<S> {
    Seeded { surface, fault: Fault::First }
}

/// `surface` with a second derivative in `u`, from `eval_second` alone, that is wrong.
pub(crate) const fn wrong_second<S>(surface: S) -> Seeded<S> {
    Seeded { surface, fault: Fault::Second }
}

/// `surface` saying it has derivatives to `order` alone.
pub(crate) const fn capped<S>(surface: S, order: u8) -> Seeded<S> {
    Seeded { surface, fault: Fault::Capped(order) }
}

/// `surface` presenting `form` rather than its own.
pub(crate) const fn wrong_form<S>(surface: S, form: Analytic) -> Seeded<S> {
    Seeded { surface, fault: Fault::Form(form) }
}

impl<S: Parametric> Parametric for Seeded<S> {
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.surface.eval_point(uv)
    }

    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let jet = self.surface.eval_first(uv);
        match self.fault {
            Fault::First => Jet1 { du: Tangent::new(jet.du.rate() + splat(SEED)), ..jet },
            Fault::Second | Fault::Form(_) | Fault::Capped(_) => jet,
        }
    }

    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let jet = self.surface.eval_second(uv);
        match self.fault {
            Fault::First => {
                Jet2 { first: Jet1 { du: Tangent::new(jet.first.du.rate() + splat(SEED)), ..jet.first }, ..jet }
            }
            Fault::Second => Jet2 { duu: Tangent::new(jet.duu.rate() + splat(SEED)), ..jet },
            Fault::Form(_) | Fault::Capped(_) => jet,
        }
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        Parametric::derivatives(&self.surface, uv, order, out, scratch)
    }

    fn domain(&self) -> Domain {
        Parametric::domain(&self.surface)
    }

    fn highest_derivative(&self) -> Option<u8> {
        match self.fault {
            Fault::Capped(order) => Some(order),
            Fault::First | Fault::Second | Fault::Form(_) => Parametric::highest_derivative(&self.surface),
        }
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        Parametric::bounds(&self.surface, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        Parametric::invert(&self.surface, p, seed, tol, scratch)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report> {
        Parametric::closest(&self.surface, p, seed, tol, scratch)
    }

    fn family(&self) -> FamilyName {
        Parametric::family(&self.surface)
    }

    fn content(&self, out: &mut Content) {
        Parametric::content(&self.surface, out);
    }

    fn shape(&self) -> Shape {
        Parametric::shape(&self.surface)
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        Parametric::degeneracies(&self.surface)
    }

    fn analytic(&self, tol: Tolerance) -> Option<Analytic> {
        match self.fault {
            Fault::Form(form) => Some(form),
            Fault::First | Fault::Second | Fault::Capped(_) => Parametric::analytic(&self.surface, tol),
        }
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

    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        let (point, rate, change) = self.0.eval_second(t);
        (point, rate, Tangent::new(change.rate() + splat(SEED)))
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

/// A line that says it was sampled, to show that the object layer passes its fit on.
pub(crate) struct Fitted(pub(crate) Line);

impl ParametricCurve for Fitted {
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
        ParametricCurve::range(&self.0)
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        ParametricCurve::bounds(&self.0, lo, hi)
    }

    fn family(&self) -> FamilyName {
        FamilyName("fitted")
    }

    fn content(&self, out: &mut Content) {
        ParametricCurve::content(&self.0, out);
    }

    fn shape(&self) -> Shape {
        ParametricCurve::shape(&self.0)
    }

    fn fit(&self) -> Option<Length> {
        Some(Length::from_millimetres(0.25))
    }
}
