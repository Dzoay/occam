//! Source: original
//! The plane: `origin + u x + v y`, where `x` and `y` are perpendicular unit axes, so both parameters are lengths.

use occam_math::Real;
use occam_report::Report;
use occam_units::{Bounds, Length, Local, ParamKind, Placement, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::analytic::Analytic;
use crate::axes::Axes;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::domain::{Domain, ParamRange, UvBox};
use crate::identity::{Content, FamilyName, Shape};
use crate::support::{enclose, exact, splat};
use crate::surface::{Closest, Parametric};

/// A plane, unbounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    axes: Axes,
}

impl Plane {
    /// The XY plane of `placement`: through its origin, with `u` along its X axis, `v` along its Y, and facing its Z.
    #[must_use]
    pub fn placed(placement: &Placement<Local, World>) -> Self {
        Self { axes: Axes::of(placement) }
    }
}

impl Parametric for Plane {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.axes.at(uv.u(), uv.v())
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let (du, dv) = (Tangent::new(splat(self.axes.x)), Tangent::new(splat(self.axes.y)));
        Jet1 { point: self.eval_point(uv), du, dv }
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let zero = Tangent::new(splat(Vec3::new(0.0, 0.0, 0.0)));
        Jet2 { first: self.eval_first(uv), duu: zero, duv: zero, dvv: zero }
    }

    fn derivatives(
        &self,
        _uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        for degree in 1..=order {
            for in_v in 0..=degree {
                let rate = match (degree, in_v) {
                    (1, 0) => self.axes.x,
                    (1, _) => self.axes.y,
                    _ => Vec3::new(0.0, 0.0, 0.0),
                };
                out.push(Tangent::new(rate));
            }
        }
        Ok(())
    }

    fn domain(&self) -> Domain {
        Domain { u: ParamRange::unbounded(ParamKind::Length), v: ParamRange::unbounded(ParamKind::Length) }
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        enclose(patch.corners().map(|corner| self.axes.reach(exact(corner.u()), exact(corner.v()))))
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let (u, v) = self.axes.coordinates(p);
        Ok(UvPoint::new(u, v))
    }

    fn closest(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<Closest, Report> {
        let uv = Parametric::invert(self, p, seed, tol, scratch)?;
        let point = self.eval_point(uv);
        Ok(Closest { at: uv, point, distance: Length::from_millimetres((p.to_vec3() - point.to_vec3()).norm()) })
    }

    fn family(&self) -> FamilyName {
        FamilyName("plane")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push(out);
    }

    fn shape(&self) -> Shape {
        Shape { form: "plane", values: self.axes.values("origin") }
    }

    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        Some(Analytic::Plane { origin: Point::from_vec3(self.axes.origin), normal: self.axes.normal })
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Level;
    use occam_report::{Code, NamedValue};
    use occam_units::Direction;

    use super::*;
    use crate::conformance::{self, Check, WrongSecond, placement, tolerance};
    use crate::surface::Surface;

    fn plane() -> Plane {
        Plane::placed(&placement())
    }

    fn window() -> UvBox {
        UvBox::new(UvPoint::new(-30.0, -20.0), UvPoint::new(40.0, 25.0))
    }

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).norm() <= 1e-12 * (1.0 + b.norm())
    }

    #[test]
    fn the_plane_passes_the_conformance_suite() {
        let failures = conformance::surface(&plane(), window());
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn the_suite_catches_a_plane_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::surface(&WrongSecond(plane()), window()));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
    }

    #[test]
    fn the_plane_of_a_placement_follows_its_axes() {
        let place = placement();
        let s = plane();
        let local = |x, y, z| place.point(Point::from_vec3(Vec3::new(x, y, z))).to_vec3();
        assert!(near(s.point(UvPoint::new(0.0, 0.0)).to_vec3(), local(0.0, 0.0, 0.0)));
        assert!(near(s.point(UvPoint::new(3.0, -2.0)).to_vec3(), local(3.0, -2.0, 0.0)));
        let jet = s.second(UvPoint::new(5.0, 7.0));
        assert!(near(jet.first.du.rate(), local(1.0, 0.0, 0.0) - local(0.0, 0.0, 0.0)));
        assert!(near(jet.first.dv.rate(), local(0.0, 1.0, 0.0) - local(0.0, 0.0, 0.0)));
        assert_eq!([jet.duu, jet.duv, jet.dvv].map(|t| t.rate().norm()), [0.0; 3]);
        let normal = place.direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let analytic = Analytic::Plane { origin: Point::from_vec3(local(0.0, 0.0, 0.0)), normal };
        assert_eq!(Surface::analytic(&s, tolerance()), Some(analytic));
    }

    #[test]
    fn its_derivatives_are_its_axes_then_zero_to_any_order() {
        let s = plane();
        let mut out = vec![Tangent::new(Vec3::new(9.0, 9.0, 9.0))];
        Surface::derivatives(&s, UvPoint::new(1.0, 2.0), 4, &mut out, &mut Scratch::default()).unwrap();
        let jet = s.first(UvPoint::new(1.0, 2.0));
        assert_eq!(out.len(), 2 + 3 + 4 + 5);
        assert_eq!((out[0], out[1]), (jet.du, jet.dv));
        assert!(out[2..].iter().all(|t| t.rate() == Vec3::new(0.0, 0.0, 0.0)));
        Surface::derivatives(&s, UvPoint::new(1.0, 2.0), 0, &mut out, &mut Scratch::default()).unwrap();
        assert_eq!(out, Vec::<Tangent<World>>::new());
        let report = Surface::derivatives(&s, UvPoint::new(f64::INFINITY, 0.0), 1, &mut out, &mut Scratch::default());
        assert_eq!(report.unwrap_err().code, Code::UndefinedParameter);
    }

    #[test]
    fn its_bounds_are_its_corners_rounded_outward() {
        let s = plane();
        let b = Surface::bounds(&s, window());
        let corners = Bounds::of(window().corners().map(|c| s.point(c))).unwrap();
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        let (corner_low, corner_high) = (corners.low().to_vec3(), corners.high().to_vec3());
        for (l, h, cl, ch) in [
            (low.x, high.x, corner_low.x, corner_high.x),
            (low.y, high.y, corner_low.y, corner_high.y),
            (low.z, high.z, corner_low.z, corner_high.z),
        ] {
            assert!(l < cl && cl - l <= 1e-12, "{l} {cl}");
            assert!(h > ch && h - ch <= 1e-12, "{h} {ch}");
        }
        let unbounded = Surface::bounds(&s, UvBox::new(UvPoint::new(f64::NEG_INFINITY, 0.0), UvPoint::new(0.0, 1.0)));
        assert!(unbounded.contains(Point::from_vec3(Vec3::new(1e300, -1e300, 1e300))));
    }

    #[test]
    fn the_closest_point_is_the_foot_of_the_perpendicular() {
        let s = plane();
        let foot = s.point(UvPoint::new(4.0, -3.0));
        let normal = place_normal();
        let above = Point::from_vec3(foot.to_vec3() + normal * 2.5);
        let c = Surface::closest(&s, above, None, tolerance(), &mut Scratch::default()).unwrap();
        assert!((c.at.u() - 4.0).abs() < 1e-12 && (c.at.v() + 3.0).abs() < 1e-12, "{:?}", c.at);
        assert!(near(c.point.to_vec3(), foot.to_vec3()));
        assert!((c.distance.millimetres() - 2.5).abs() < 1e-12);
        let refused = Surface::invert(
            &s,
            Point::from_vec3(Vec3::new(f64::NAN, 0.0, 0.0)),
            None,
            tolerance(),
            &mut Scratch::default(),
        );
        assert_eq!(refused.unwrap_err().code, Code::NotFinite);
    }

    fn place_normal() -> Vec3 {
        placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).get()
    }

    #[test]
    fn its_shape_and_content_name_what_defines_it() {
        let s = plane();
        let o = s.point(UvPoint::new(0.0, 0.0)).to_vec3();
        let shape = Surface::shape(&s);
        assert_eq!(shape.form, "plane");
        assert_eq!(shape.values[0], NamedValue::point("origin", [o.x, o.y, o.z]));
        assert_eq!(shape.values[1], crate::axes::direction("normal", place_normal()));
        assert_eq!(shape.values[2], crate::axes::direction("x-axis", s.first(UvPoint::new(0.0, 0.0)).du.rate()));
        assert_eq!(shape.values.len(), 3);
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(content.words().len(), 1 + 5 + 12);
        let mut other = Content::default();
        Surface::content(&Plane::placed(&Placement::identity()), &mut other);
        assert_ne!(content, other);
        assert_eq!(Surface::batch_key(&s), crate::BatchKey { family: FamilyName("plane"), degree: (0, 0) });
        assert_eq!(Surface::degeneracies(&s), &[]);
        assert!(Surface::to_bspline(&s).is_none() && Surface::triangles(&s).is_none());
        let mut jets = crate::Jets::default();
        Surface::eval_batch(
            &s,
            &[UvPoint::new(1.0, 1.0)],
            crate::Order::Point,
            &mut jets,
            &mut Scratch::default(),
            Level::baseline(),
        );
        assert_eq!(jets.points, [s.point(UvPoint::new(1.0, 1.0))]);
    }
}
