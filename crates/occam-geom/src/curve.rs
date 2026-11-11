//! Source: original
//! The curve seam: the surface's two layers over one parameter, [`ParametricCurve`] for a family and [`Curve`] for
//! everyone else, and [`ParamCurve`] for a curve in a face's parameters.

use occam_math::Real;
use occam_report::Report;
use occam_units::{Bounds, Length, Point, Tangent, Tolerance, UvPoint, World};

use crate::batch::Scratch;
use crate::domain::ParamRange;
use crate::identity::{Content, FamilyName, Shape};
use crate::support::finite;

/// The first layer, which a curve family implements, generic over [`Real`]. As for [`crate::Parametric`], the `eval_*`
/// methods are total and give NaN where the curve is undefined.
pub trait ParametricCurve: Sized + Send + Sync + 'static {
    /// The point.
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R>;

    /// The point and its derivative.
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>);

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

    /// The parameter of the foot of `p`, which is finite.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance, scratch: &mut Scratch) -> Result<f64, Report>;

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
}

/// The second layer for curves: dyn-compatible, and checking each call as [`crate::Surface`] does.
pub trait Curve: Send + Sync {
    /// As [`ParametricCurve::eval_point`].
    fn point(&self, t: f64) -> Point<World>;

    /// As [`ParametricCurve::eval_first`].
    fn first(&self, t: f64) -> (Point<World>, Tangent<World>);

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
}

impl<C: ParametricCurve> Curve for C {
    fn point(&self, t: f64) -> Point<World> {
        self.eval_point(t)
    }

    fn first(&self, t: f64) -> (Point<World>, Tangent<World>) {
        self.eval_first(t)
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
}

/// A curve in a face's parameters (S1 item 7: without one, the same answers took 3.45 times as long; PARITY 2.3). Its
/// families, and the family layer beneath this one, come with `geometry.parameter`.
pub trait ParamCurve: Send + Sync {
    /// The point in the face's parameters.
    fn uv(&self, t: f64) -> UvPoint;

    /// Where it is defined.
    fn range(&self) -> ParamRange;

    /// The family's name.
    fn family(&self) -> FamilyName;

    /// Replaces `out` with the family's name and the numbers that define the curve.
    fn content(&self, out: &mut Content);

    /// How closely a sampled parameter curve fits, in the surface's parameters. `None` for one that is exact.
    fn fit(&self) -> Option<f64>;
}

#[cfg(test)]
mod tests {
    use occam_units::{Direction, Vec3};

    use super::*;
    use crate::line::Line;

    /// A line that says it was sampled, to show that the object layer passes its fit on.
    struct Fitted(Line);

    impl ParametricCurve for Fitted {
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
            ParametricCurve::derivatives(&self.0, t, order, out, scratch)
        }

        fn range(&self) -> ParamRange {
            ParametricCurve::range(&self.0)
        }

        fn bounds(&self, lo: f64, hi: f64) -> Bounds {
            ParametricCurve::bounds(&self.0, lo, hi)
        }

        fn invert(
            &self,
            p: Point<World>,
            seed: Option<f64>,
            tol: Tolerance,
            scratch: &mut Scratch,
        ) -> Result<f64, Report> {
            ParametricCurve::invert(&self.0, p, seed, tol, scratch)
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

    #[test]
    fn the_object_layer_passes_on_a_fit() {
        let line =
            Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let c: &dyn Curve = &Fitted(line);
        assert_eq!(c.fit(), Some(Length::from_millimetres(0.25)));
        assert_eq!(c.family(), FamilyName("fitted"));
    }
}
