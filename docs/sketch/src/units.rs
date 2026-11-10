//! `occam-units`: what it gains. Conversion at the API is unchanged: a request carries its units, and `Length::new` and
//! `Angle::new` remain the one place numbers become quantities.

use std::marker::PhantomData;

use occam_math::Real;
use occam_report::Report;
use occam_units::{Frame, Length, Point, Tolerance, Vector, World};

/// Three plain numbers: the arithmetic inside a formula, typed again at its ends. S1 found the typed layer stopped where
/// formulas began, for want of dot, cross and norm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3<R: Real = f64> {
    pub x: R,
    pub y: R,
    pub z: R,
}

impl<R: Real> Vec3<R> {
    pub fn dot(self, other: Self) -> R {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn norm(self) -> R {
        self.dot(self).sqrt()
    }
}

/// A dimensionless unit axis (S4): not a `Vector`, which is three lengths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction<F: Frame, R: Real = f64> {
    unit: Vec3<R>,
    frame: PhantomData<F>,
}

impl<F: Frame> Direction<F> {
    /// # Errors
    ///
    /// `geom.degenerate-direction` if `v` is no longer than the tolerance, or is not finite.
    pub fn along(v: Vector<F>, tolerance: Tolerance) -> Result<Self, Report> {
        unimplemented!()
    }

    #[must_use]
    pub const fn get(self) -> Vec3 {
        self.unit
    }
}

/// A derivative: length per unit of a parameter that may be a length, an angle or a knot unit (S4). Not three lengths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tangent<F: Frame, R: Real = f64> {
    pub rate: Vec3<R>,
    frame: PhantomData<F>,
}

impl<F: Frame, R: Real> Tangent<F, R> {
    pub const fn new(rate: Vec3<R>) -> Self {
        Self { rate, frame: PhantomData }
    }
}

/// What a surface or curve parameter measures, recorded in its domain, so a test in parameter space uses the right
/// tolerance (S4's cone compared a length with an angle tolerance). Parameters themselves stay bare numbers, so lane
/// kernels read plain `f64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Length,
    Angle,
    Knot,
}

/// A rigid map from `A` to `B`, built once with the owned `sin_cos`. Inputs arrive in `Local` and are placed into `World`,
/// where every stored curve and surface lives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement<A: Frame, B: Frame> {
    rows: [Vec3; 3],
    origin: Point<B>,
    from: PhantomData<A>,
}

impl<A: Frame, B: Frame> Placement<A, B> {
    pub fn point(&self, p: Point<A>) -> Point<B> {
        unimplemented!()
    }

    pub fn direction(&self, d: Direction<A>) -> Direction<B> {
        unimplemented!()
    }
}

/// A box that holds everything it bounds, rounding included (S1 item 13, P5, P8): a box too small is a silent wrong
/// answer, so each bound is tested against the unculled run's digest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point<World>,
    pub max: Point<World>,
}

impl Bounds {
    #[must_use]
    pub fn overlaps(&self, other: &Self, tolerance: Tolerance) -> bool {
        unimplemented!()
    }
}

/// CHANGED: what `Tolerance` gains (S2). The same two numbers per model, never per entity, with a stated meaning.
pub trait ToleranceNext {
    /// What every decision compares with, exactly: `|q| <= band` is on, tangent or the same. It never grows.
    fn band(self) -> Length;
    /// How far a band decision at near tangency moves a vertex on a curvature radius `radius`: `sqrt(2 R eps)` (S2 item
    /// 4). It goes into the report as a named value, never into the vertex's certified radius, which would bring back the
    /// hull S2's snapping drops (critique of design A, point 2).
    fn near_tangent_shift(self, radius: Length) -> Length;
    /// The band in a parameter of this kind at `rate` of length per unit (S4's cone).
    fn in_parameter(self, kind: ParamKind, rate: Length) -> f64;
}

impl ToleranceNext for Tolerance {
    fn band(self) -> Length {
        self.length()
    }

    fn near_tangent_shift(self, radius: Length) -> Length {
        unimplemented!()
    }

    fn in_parameter(self, kind: ParamKind, rate: Length) -> f64 {
        unimplemented!()
    }
}
