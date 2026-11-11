//! Source: original
//! The surface seam's two layers (S4): [`Parametric`], which a family implements generic over [`Real`], and [`Surface`],
//! which everything else calls. One blanket impl joins them, and checks each call on the way.

use occam_math::{Level, Real};
use occam_report::Report;
use occam_units::{Bounds, Length, Point, Tangent, Tolerance, UvPoint, World};

use crate::analytic::Analytic;
use crate::batch::{Jet1, Jet2, Jets, Order, Scratch, eval_in_lanes};
use crate::domain::{Degeneracy, Domain, UvBox};
use crate::identity::{BatchKey, Content, FamilyName, Shape};
use crate::support::finite;

/// The closest point of a surface to a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closest {
    /// Its parameters.
    pub uv: UvPoint,
    /// The point.
    pub point: Point<World>,
    /// How far it is.
    pub distance: Length,
}

/// A rational B-spline surface, as a family presents itself where it has one exactly (PARITY 2.2).
#[derive(Clone, Debug, PartialEq)]
pub struct BsplineSurface {
    /// The degree in each parameter.
    pub degree: (u16, u16),
    /// How many poles in each parameter.
    pub count: (u32, u32),
    /// The knots in `u`.
    pub knots_u: Vec<f64>,
    /// The knots in `v`.
    pub knots_v: Vec<f64>,
    /// The weighted poles, `count.0 * count.1` of them, a row of constant `v` at a time.
    pub poles: Vec<[f64; 4]>,
}

/// What a mesh-backed surface presents (S4, L21).
pub trait TriangleView: Send + Sync {
    /// How many triangles.
    fn triangle_count(&self) -> usize;

    /// One triangle's corners, or `None` past the last.
    fn triangle(&self, index: usize) -> Option<[Point<World>; 3]>;
}

/// The first layer, which a family implements: generic over [`Real`], so that one formula runs as `f64` or in lanes.
///
/// The `eval_*` methods are total: where the surface is undefined they give NaN in that lane, and nothing beyond the
/// order asked for is computed. Anything that depends only on the surface is computed when it is made (S4's cone).
pub trait Parametric: Sized + Send + Sync + 'static {
    /// The point.
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R>;

    /// The point and its first derivatives.
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R>;

    /// The point and its first and second derivatives.
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R>;

    /// Appends every partial derivative of total degree 1 to `order` at `uv`, which is inside the domain: by degree,
    /// and within a degree from all `u` to all `v`, so `du, dv, duu, duv, dvv, duuu` and on. Exact where the family can
    /// be. PARITY 2.1 and 2.2 ask for any order (O21); the hot paths use the `eval_*` rungs.
    ///
    /// # Errors
    ///
    /// Where the family has no derivative of an order asked for.
    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report>;

    /// The surface at many points, into the caller's `out`. This runs [`eval_in_lanes`] at width 1; a family that has
    /// measured a gain in lanes overrides it to choose its width.
    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level) {
        let _ = scratch;
        eval_in_lanes::<1, Self>(self, uv, order, out, level);
    }

    /// Where it is defined.
    fn domain(&self) -> Domain;

    /// A box holding the surface over `patch`, rounding included.
    fn bounds(&self, patch: UvBox) -> Bounds;

    /// The parameters of the foot of `p`, which is finite, without derivatives (P8: half of a point query). Required, so
    /// that no family inherits the full solve.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report>;

    /// The point of the surface closest to `p`, which is finite.
    ///
    /// # Errors
    ///
    /// Where the solver stopped, carried in the report (L5).
    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report>;

    /// The family's name.
    fn family(&self) -> FamilyName;

    /// What can share a sorted run of calls with it. A family with a degree overrides this to give it.
    fn batch_key(&self) -> BatchKey {
        BatchKey { family: self.family(), degree: (0, 0) }
    }

    /// Adds the numbers that define it to `out`, which already holds the family's name.
    fn content(&self, out: &mut Content);

    /// The form and parameters a description shows.
    fn shape(&self) -> Shape;

    /// The lines that collapse to a point, declared when it was made.
    fn degeneracies(&self) -> &[Degeneracy] {
        &[]
    }

    /// The exact form it is, if it holds one within `tol`. What a surface can present is asked for, never what it is,
    /// so each of these defaults to nothing and a new family edits no list (S4).
    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        None
    }

    /// The rational B-spline it is exactly, if it has one.
    fn to_bspline(&self) -> Option<BsplineSurface> {
        None
    }

    /// Its triangles, if it is made of them.
    fn triangles(&self) -> Option<&dyn TriangleView> {
        None
    }
}

/// The second layer, which everything outside a family calls: dyn-compatible, `f64` per call, and lanes inside a batch.
pub trait Surface: Send + Sync {
    /// As [`Parametric::eval_point`].
    fn point(&self, uv: UvPoint) -> Point<World>;

    /// As [`Parametric::eval_first`].
    fn first(&self, uv: UvPoint) -> Jet1;

    /// As [`Parametric::eval_second`].
    fn second(&self, uv: UvPoint) -> Jet2;

    /// Replaces `out` with every partial derivative of total degree 1 to `order`, as [`Parametric::derivatives`] orders
    /// them.
    ///
    /// # Errors
    ///
    /// `geom.undefined-parameter` outside the domain, and what [`Parametric::derivatives`] reports.
    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report>;

    /// As [`Parametric::eval_batch`].
    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level);

    /// As [`Parametric::closest`].
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point that is not finite, and what [`Parametric::closest`] reports.
    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report>;

    /// As [`Parametric::invert`].
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point that is not finite, and what [`Parametric::invert`] reports.
    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report>;

    /// As [`Parametric::domain`].
    fn domain(&self) -> Domain;

    /// As [`Parametric::bounds`].
    fn bounds(&self, patch: UvBox) -> Bounds;

    /// As [`Parametric::family`].
    fn family(&self) -> FamilyName;

    /// As [`Parametric::batch_key`].
    fn batch_key(&self) -> BatchKey;

    /// Replaces `out` with the family's name and the numbers that define the surface.
    fn content(&self, out: &mut Content);

    /// As [`Parametric::shape`].
    fn shape(&self) -> Shape;

    /// As [`Parametric::degeneracies`].
    fn degeneracies(&self) -> &[Degeneracy];

    /// As [`Parametric::analytic`].
    fn analytic(&self, tol: Tolerance) -> Option<Analytic>;

    /// As [`Parametric::to_bspline`].
    fn to_bspline(&self) -> Option<BsplineSurface>;

    /// As [`Parametric::triangles`].
    fn triangles(&self) -> Option<&dyn TriangleView>;
}

/// The one place the family trait becomes the object trait.
impl<S: Parametric> Surface for S {
    fn point(&self, uv: UvPoint) -> Point<World> {
        self.eval_point(uv)
    }

    fn first(&self, uv: UvPoint) -> Jet1 {
        self.eval_first(uv)
    }

    fn second(&self, uv: UvPoint) -> Jet2 {
        self.eval_second(uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        out.clear();
        Parametric::domain(self).check(uv)?;
        Parametric::derivatives(self, uv, order, out, scratch)
    }

    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level) {
        Parametric::eval_batch(self, uv, order, out, scratch, level);
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report> {
        finite(p)?;
        Parametric::closest(self, p, seed, tol, scratch)
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        finite(p)?;
        Parametric::invert(self, p, seed, tol, scratch)
    }

    fn domain(&self) -> Domain {
        Parametric::domain(self)
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        Parametric::bounds(self, patch)
    }

    fn family(&self) -> FamilyName {
        Parametric::family(self)
    }

    fn batch_key(&self) -> BatchKey {
        Parametric::batch_key(self)
    }

    fn content(&self, out: &mut Content) {
        out.start(Parametric::family(self));
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

    fn to_bspline(&self) -> Option<BsplineSurface> {
        Parametric::to_bspline(self)
    }

    fn triangles(&self) -> Option<&dyn TriangleView> {
        Parametric::triangles(self)
    }
}

#[cfg(test)]
mod tests {
    use occam_units::Vec3;

    use super::*;
    use crate::conformance::{placement, tolerance};
    use crate::domain::Param;
    use crate::plane::Plane;

    /// A plane that presents everything a family can, to show that the object layer passes each on.
    struct Presenting {
        plane: Plane,
        lines: [Degeneracy; 1],
    }

    fn presenting() -> Presenting {
        let plane = Plane::placed(&placement());
        let point = plane.eval_point(UvPoint::new(0.0, 0.0));
        Presenting { plane, lines: [Degeneracy { param: Param::V, at: 0.0, point, normal: None }] }
    }

    fn bspline() -> BsplineSurface {
        let poles = vec![[0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0]];
        BsplineSurface {
            degree: (1, 1),
            count: (2, 2),
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            poles,
        }
    }

    impl TriangleView for Presenting {
        fn triangle_count(&self) -> usize {
            1
        }

        fn triangle(&self, index: usize) -> Option<[Point<World>; 3]> {
            let corner = |u, v| self.plane.eval_point(UvPoint::new(u, v));
            (index == 0).then(|| [corner(0.0, 0.0), corner(1.0, 0.0), corner(0.0, 1.0)])
        }
    }

    impl Parametric for Presenting {
        fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
            self.plane.eval_point(uv)
        }

        fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
            self.plane.eval_first(uv)
        }

        fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
            self.plane.eval_second(uv)
        }

        fn derivatives(
            &self,
            uv: UvPoint,
            order: u8,
            out: &mut Vec<Tangent<World>>,
            scratch: &mut Scratch,
        ) -> Result<(), Report> {
            Parametric::derivatives(&self.plane, uv, order, out, scratch)
        }

        fn domain(&self) -> Domain {
            Parametric::domain(&self.plane)
        }

        fn bounds(&self, patch: UvBox) -> Bounds {
            Parametric::bounds(&self.plane, patch)
        }

        fn invert(
            &self,
            p: Point<World>,
            seed: Option<UvPoint>,
            tol: Tolerance,
            scratch: &mut Scratch,
        ) -> Result<UvPoint, Report> {
            Parametric::invert(&self.plane, p, seed, tol, scratch)
        }

        fn closest(
            &self,
            p: Point<World>,
            seed: Option<UvPoint>,
            tol: Tolerance,
            scratch: &mut Scratch,
        ) -> Result<Closest, Report> {
            Parametric::closest(&self.plane, p, seed, tol, scratch)
        }

        fn family(&self) -> FamilyName {
            FamilyName("presenting")
        }

        fn batch_key(&self) -> BatchKey {
            BatchKey { family: Parametric::family(self), degree: (1, 1) }
        }

        fn content(&self, out: &mut Content) {
            Parametric::content(&self.plane, out);
        }

        fn shape(&self) -> Shape {
            Parametric::shape(&self.plane)
        }

        fn degeneracies(&self) -> &[Degeneracy] {
            &self.lines
        }

        fn to_bspline(&self) -> Option<BsplineSurface> {
            Some(bspline())
        }

        fn triangles(&self) -> Option<&dyn TriangleView> {
            Some(self)
        }
    }

    #[test]
    fn the_object_layer_passes_on_what_a_family_presents() {
        let family = presenting();
        let s: &dyn Surface = &family;
        assert_eq!(s.degeneracies(), family.lines);
        assert_eq!(s.to_bspline(), Some(bspline()));
        let view = s.triangles().unwrap();
        assert_eq!((view.triangle_count(), view.triangle(1)), (1, None));
        assert_eq!(view.triangle(0).map(|t| t[1]), Some(family.plane.eval_point(UvPoint::new(1.0, 0.0))));
        assert_eq!(s.batch_key(), BatchKey { family: FamilyName("presenting"), degree: (1, 1) });
        assert_eq!(s.analytic(tolerance()), None, "the default presents no exact form");
        let p = Point::from_vec3(Vec3::new(1.0, 2.0, 3.0));
        let (uv, mut scratch) = (s.invert(p, None, tolerance(), &mut scratch_of()).unwrap(), scratch_of());
        assert_eq!(s.closest(p, None, tolerance(), &mut scratch).unwrap().uv, uv);
    }

    fn scratch_of() -> Scratch {
        Scratch::default()
    }
}
