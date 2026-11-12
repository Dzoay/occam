//! `occam-geom`: curves, parameter curves and surfaces in S4's two layers. All stored geometry is in `World`: inputs are
//! placed there once, so the traits carry no frame parameter, and S4's E0119 case (an object-only family generic over
//! its frame) cannot arise. There is no cross-family column store: batches run over one surface at many points, and
//! calls are sorted by family and degree (O23).

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Angle, Length, Point, Tolerance, UvPoint, World};

use occam_math::Level;
use crate::units::{Bounds, Direction, ParamKind, Tangent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    Open,
    Closed,
    Periodic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamRange {
    pub lo: f64,
    pub hi: f64,
    pub wrap: Wrap,
    pub kind: ParamKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain {
    pub u: ParamRange,
    pub v: ParamRange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvBox {
    pub lo: UvPoint,
    pub hi: UvPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    U,
    V,
}

/// A parameter line that collapses to a point, declared at construction. A sphere's pole has a normal there and a cone's
/// apex does not (S4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Degeneracy {
    pub param: Param,
    pub at: f64,
    pub point: Point<World>,
    pub normal: Option<Direction<World>>,
}

/// A family's stable name. The canonical pair order depends on it (S4), so a test holds every registered name unique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FamilyName(pub &'static str);

/// What can share a lane loop and a sorted run of calls (L29, as O23 narrows it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchKey {
    pub family: FamilyName,
    pub degree: (u16, u16),
}

/// The canonical bits of everything that defines a curve or surface, family name first. The pair order and a later
/// cache key compare these words, never a pointer (P8). No rounding.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Content(Vec<u64>);

impl Content {
    pub fn push(&mut self, x: f64) {
        self.0.push(occam_math::canonical_bits(x));
    }
}

/// What a curve or surface is in the caller's terms: a form name from the catalogue and its parameters (S6, O26).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub form: &'static str,
    pub values: Vec<NamedValue>,
}

/// The rungs the hot paths evaluate. Nothing beyond the order asked for is computed (P8 item 1: 33 to 34% of whole-run
/// time on the two workloads timed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Order {
    Point,
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet1<R: Real = f64> {
    pub point: Point<World, R>,
    pub du: Tangent<World, R>,
    pub dv: Tangent<World, R>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet2<R: Real = f64> {
    pub first: Jet1<R>,
    pub duu: Tangent<World, R>,
    pub duv: Tangent<World, R>,
    pub dvv: Tangent<World, R>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closest {
    pub uv: UvPoint,
    pub point: Point<World>,
    pub distance: Length,
}

/// The exact forms, presented by whatever family holds one: the vocabulary of the closed-form intersections (P5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Analytic {
    Plane { origin: Point<World>, normal: Direction<World> },
    Cylinder { on_axis: Point<World>, axis: Direction<World>, radius: Length },
    Cone { apex: Point<World>, axis: Direction<World>, half_angle: Angle },
    Sphere { centre: Point<World>, radius: Length },
    Torus { centre: Point<World>, axis: Direction<World>, major: Length, minor: Length },
}

/// A rational B-spline surface: what conversion gives (PARITY 2.2), where a family has one exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct BsplineSurface {
    pub degree: (u16, u16),
    pub count: (u32, u32),
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
    /// `count.0 * count.1` poles, row by row, and a weight for each where it is rational.
    pub poles: Vec<Point<World>>,
    pub weights: Option<Vec<f64>>,
}

/// What a mesh-backed surface presents (S4, L21).
pub trait TriangleView: Send + Sync {
    fn triangle_count(&self) -> usize;
    fn triangle(&self, index: usize) -> Option<[Point<World>; 3]>;
}

/// Which items of a batch were defined: L6's result per item, which S4 could reach only by scanning for NaN.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Defined {
    bits: Vec<u64>,
    len: usize,
}

impl Defined {
    #[must_use]
    pub fn first_undefined(&self) -> Option<usize> {
        unimplemented!()
    }
}

/// The caller's workspace, so that no evaluation allocates (S4: degree 5 by 4 allocated on every call, 370 ns).
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    reals: Vec<f64>,
    indices: Vec<u32>,
}

/// A batch's outputs, owned by the caller and written only up to the order asked for.
#[derive(Clone, Debug, Default)]
pub struct Jets {
    pub points: Vec<Point<World>>,
    pub du: Vec<Tangent<World>>,
    pub dv: Vec<Tangent<World>>,
    pub duu: Vec<Tangent<World>>,
    pub duv: Vec<Tangent<World>>,
    pub dvv: Vec<Tangent<World>>,
}

/// Layer 1, what a family implements: generic over `Real`, so one formula runs as `f64` or in lanes. `eval_*` is total:
/// an undefined parameter is NaN in that lane. Anything that depends only on the surface is computed at construction
/// (S4's cone).
pub trait Parametric: Sized + Send + Sync + 'static {
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R>;
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R>;
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R>;

    /// Every partial derivative to total order `order`, by total degree, exact where the family can be. PARITY 2.1 and
    /// 2.2 ask for any derivative (O21); the hot paths use the three rungs.
    ///
    /// # Errors
    ///
    /// `geom.undefined-parameter` where the surface is undefined at `uv`.
    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report>;

    /// One surface at many points: P1's sampling, P9's refinement and P12's dirty rectangles. The default runs the scalar
    /// rungs. A family overrides it to choose its lane width per order and to call `dispatch` inside, since dispatch
    /// cannot cross `dyn` (P4).
    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level) -> Defined {
        unimplemented!()
    }

    fn domain(&self) -> Domain;

    /// `None` where every order exists; a mesh's is 1, and `derivatives` refuses an order above it.
    fn highest_derivative(&self) -> Option<u8> {
        None
    }

    /// Conservative, rounding included.
    fn bounds(&self, patch: UvBox) -> Bounds;

    /// The parameters of `p`'s foot, without derivatives (P8: half of a point query). Required, so that no family
    /// inherits the full solve by default.
    ///
    /// # Errors
    ///
    /// A report carrying where the solver stopped (L5).
    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report>;

    /// # Errors
    ///
    /// A report carrying where the solver stopped (L5).
    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report>;

    fn family(&self) -> FamilyName;
    fn batch_key(&self) -> BatchKey;
    fn content(&self, out: &mut Content);
    fn shape(&self) -> Shape;
    fn degeneracies(&self) -> &[Degeneracy];

    // What the surface can present, asked instead of what it is. Each defaults to "nothing", so a new family edits no
    // list (S4).
    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        None
    }

    /// Over a patch, never an unbounded one (docs/decisions/2026-11-11-to-bspline-over-a-range.md).
    fn to_bspline(&self, _patch: UvBox) -> Option<BsplineSurface> {
        None
    }

    fn triangles(&self) -> Option<&dyn TriangleView> {
        None
    }
}

/// Layer 2, what everything outside a family calls: dyn-compatible, `f64` per call, lanes inside a batch.
pub trait Surface: Send + Sync {
    fn point(&self, uv: UvPoint) -> Point<World>;
    fn first(&self, uv: UvPoint) -> Jet1;
    fn second(&self, uv: UvPoint) -> Jet2;

    /// # Errors
    ///
    /// As [`Parametric::derivatives`].
    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report>;

    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level) -> Defined;
    fn domain(&self) -> Domain;
    fn highest_derivative(&self) -> Option<u8>;
    fn bounds(&self, patch: UvBox) -> Bounds;

    /// # Errors
    ///
    /// As [`Parametric::invert`].
    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report>;

    /// # Errors
    ///
    /// As [`Parametric::closest`].
    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report>;

    fn family(&self) -> FamilyName;
    fn batch_key(&self) -> BatchKey;
    fn content(&self, out: &mut Content);
    fn shape(&self) -> Shape;
    fn degeneracies(&self) -> &[Degeneracy];
    fn analytic(&self, tol: Tolerance) -> Option<Analytic>;
    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface>;
    fn triangles(&self) -> Option<&dyn TriangleView>;
}

/// The one place the family trait becomes the object trait.
impl<S: Parametric> Surface for S {
    fn point(&self, uv: UvPoint) -> Point<World> {
        self.eval_point::<f64>(uv)
    }

    fn first(&self, uv: UvPoint) -> Jet1 {
        self.eval_first::<f64>(uv)
    }

    fn second(&self, uv: UvPoint) -> Jet2 {
        self.eval_second::<f64>(uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        Parametric::derivatives(self, uv, order, out, scratch)
    }

    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level) -> Defined {
        Parametric::eval_batch(self, uv, order, out, scratch, level)
    }

    fn domain(&self) -> Domain {
        Parametric::domain(self)
    }

    fn highest_derivative(&self) -> Option<u8> {
        Parametric::highest_derivative(self)
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        Parametric::bounds(self, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        Parametric::invert(self, p, seed, tol, scratch)
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report> {
        Parametric::closest(self, p, seed, tol, scratch)
    }

    fn family(&self) -> FamilyName {
        Parametric::family(self)
    }

    fn batch_key(&self) -> BatchKey {
        Parametric::batch_key(self)
    }

    fn content(&self, out: &mut Content) {
        Parametric::content(self, out);
    }

    fn shape(&self) -> Shape {
        Parametric::shape(self)
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        Parametric::degeneracies(self)
    }

    fn analytic(&self, tol: Tolerance) -> Option<Analytic> {
        Parametric::analytic(self, tol)
    }

    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        Parametric::to_bspline(self, patch)
    }

    fn triangles(&self) -> Option<&dyn TriangleView> {
        Parametric::triangles(self)
    }
}

/// Layer 1 for curves: the same pattern over one parameter.
pub trait ParametricCurve: Sized + Send + Sync + 'static {
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R>;
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>);

    /// # Errors
    ///
    /// `geom.undefined-parameter` where the curve is undefined at `t`.
    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<Tangent<World>>) -> Result<(), Report>;
    fn range(&self) -> ParamRange;
    fn bounds(&self, lo: f64, hi: f64) -> Bounds;

    /// # Errors
    ///
    /// A report carrying where the solver stopped (L5).
    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance) -> Result<f64, Report>;
    fn family(&self) -> FamilyName;
    fn content(&self, out: &mut Content);
    fn shape(&self) -> Shape;

    /// A sampled curve's achieved fit, which S1 found it must state (item 10). `None` for an exact curve.
    fn fit(&self) -> Option<Length> {
        None
    }
}

/// Layer 2 for curves.
pub trait Curve: Send + Sync {
    fn point(&self, t: f64) -> Point<World>;
    fn first(&self, t: f64) -> (Point<World>, Tangent<World>);

    /// # Errors
    ///
    /// As [`ParametricCurve::derivatives`].
    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<Tangent<World>>) -> Result<(), Report>;
    fn range(&self) -> ParamRange;
    fn bounds(&self, lo: f64, hi: f64) -> Bounds;

    /// # Errors
    ///
    /// As [`ParametricCurve::invert`].
    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance) -> Result<f64, Report>;
    fn family(&self) -> FamilyName;
    fn content(&self, out: &mut Content);
    fn shape(&self) -> Shape;
    fn fit(&self) -> Option<Length>;
}

impl<C: ParametricCurve> Curve for C {
    fn point(&self, t: f64) -> Point<World> {
        self.eval_point::<f64>(t)
    }

    fn first(&self, t: f64) -> (Point<World>, Tangent<World>) {
        self.eval_first::<f64>(t)
    }

    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<Tangent<World>>) -> Result<(), Report> {
        ParametricCurve::derivatives(self, t, order, out)
    }

    fn range(&self) -> ParamRange {
        ParametricCurve::range(self)
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        ParametricCurve::bounds(self, lo, hi)
    }

    fn invert(&self, p: Point<World>, seed: Option<f64>, tol: Tolerance) -> Result<f64, Report> {
        ParametricCurve::invert(self, p, seed, tol)
    }

    fn family(&self) -> FamilyName {
        ParametricCurve::family(self)
    }

    fn content(&self, out: &mut Content) {
        ParametricCurve::content(self, out);
    }

    fn shape(&self) -> Shape {
        ParametricCurve::shape(self)
    }

    fn fit(&self) -> Option<Length> {
        ParametricCurve::fit(self)
    }
}

/// A curve in a face's parameters (S1 item 7: without one, the same answers 3.45 times slower; PARITY 2.3). It has the
/// same two layers as `Curve`; only the object layer is shown.
pub trait ParamCurve: Send + Sync {
    fn uv(&self, t: f64) -> UvPoint;
    fn range(&self) -> ParamRange;
    fn content(&self, out: &mut Content);
    /// A sampled parameter curve's achieved fit, in the surface's parameters.
    fn fit(&self) -> Option<f64>;
}
