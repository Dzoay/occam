//! Source: original
//! A curve in a face's parameters as a curve in space, `(u, v)` at the point `(u, v, 0)`: an exact embedding, so the
//! suite's curve checks run over a parameter curve unchanged, each through the parameter curve's own answer. A point
//! off the plane has, as its foot, the foot of its shadow on it, and its distance takes in its height.

use occam_report::Report;
use occam_units::{Bounds, Length, Point, Tangent, Tolerance, UvPoint, UvVector, Vec3, World};

use super::{Failure, curve};
use crate::support::{everything, finite};
use crate::{BsplineCurve, Closest, Content, Curve, FamilyName, ParamCurve, ParamRange, Scratch, Shape};

/// A parameter curve read as a curve in the plane `z = 0`.
struct Lifted<'a>(&'a dyn ParamCurve);

/// The point of the plane at `uv`.
fn up(uv: UvPoint) -> Point<World> {
    Point::from_vec3(Vec3::new(uv.u(), uv.v(), 0.0))
}

/// The rate in the plane along `d`.
fn rate(d: UvVector) -> Tangent<World> {
    Tangent::new(Vec3::new(d.u(), d.v(), 0.0))
}

/// The place in the plane under `p`.
fn under(p: Point<World>) -> UvPoint {
    let v = p.to_vec3();
    UvPoint::new(v.x, v.y)
}

impl Curve for Lifted<'_> {
    fn point(&self, t: f64) -> Point<World> {
        up(self.0.uv(t))
    }

    fn first(&self, t: f64) -> (Point<World>, Tangent<World>) {
        let (uv, d) = self.0.first(t);
        (up(uv), rate(d))
    }

    fn second(&self, t: f64) -> (Point<World>, Tangent<World>, Tangent<World>) {
        let (uv, d1, d2) = self.0.second(t);
        (up(uv), rate(d1), rate(d2))
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let mut rates = vec![];
        let asked = self.0.derivatives(t, order, &mut rates, scratch);
        out.clear();
        out.extend(rates.into_iter().map(rate));
        asked
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        _tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest<f64>, Report> {
        finite(p)?;
        let foot = self.0.closest(under(p), seed, scratch)?;
        let point = up(foot.uv);
        Ok(Closest { at: foot.at, point, distance: Length::from_millimetres((point.to_vec3() - p.to_vec3()).norm()) })
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<f64>,
        _tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<f64, Report> {
        finite(p)?;
        self.0.invert(under(p), seed, scratch)
    }

    fn range(&self) -> ParamRange {
        self.0.range()
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let b = self.0.bounds(lo, hi);
        Bounds::of([up(b.lo), up(b.hi)]).unwrap_or_else(everything)
    }

    fn family(&self) -> FamilyName {
        self.0.family()
    }

    fn content(&self, out: &mut Content) {
        self.0.content(out);
    }

    fn shape(&self) -> Shape {
        self.0.shape()
    }

    fn fit(&self) -> Option<Length> {
        self.0.fit().map(Length::from_millimetres)
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        self.0.to_bspline(lo, hi)
    }
}

/// Every check of the parameter curve `c` from `lo` to `hi`, which must lie in its range, through the curve suite: what
/// failed, or nothing.
pub(crate) fn param_curve(c: &dyn ParamCurve, lo: f64, hi: f64) -> Vec<Failure> {
    curve(&Lifted(c), lo, hi)
}
