//! Source: original
//! The sphere: `centre + r cos v (cos u x + sin u y) + r sin v z`, where `u` is an angle that comes round and `v` the
//! latitude, from the pole at −π/2 to the pole at π/2 (PARITY 1.5). Its foot is closed-form: the angles of the point
//! about the centre.

use std::f64::consts::FRAC_PI_2;

use occam_math::{Real, atan2};
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Local, ParamKind, Placement, Point, Tangent, Tolerance, UvPoint, World};

use crate::analytic::Analytic;
use crate::axes::Axes;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::domain::{Degeneracy, Domain, Param, ParamRange, UvBox, Wrap};
use crate::identity::{Content, FamilyName, Shape};
use crate::meridian::{self, Meridian, cylindrical, turned_box};
use crate::support::positive;
use crate::surface::Parametric;
use crate::turn::turn;

/// A sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    axes: Axes,
    radius: f64,
    /// The poles, where the lines of constant `v` collapse to points that have a normal.
    poles: [Degeneracy; 2],
}

impl Sphere {
    /// The sphere of `radius` about the origin of `placement`: `u` from its X axis towards its Y, and `v` from its XY
    /// plane towards its Z, the poles on its Z axis.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length greater than zero.
    pub fn placed(placement: &Placement<Local, World>, radius: Length) -> Result<Self, Report> {
        let (axes, radius) = (Axes::of(placement), positive(radius)?);
        let pole = |at: f64, normal: occam_units::Direction<World>| Degeneracy {
            param: Param::V,
            at,
            point: Point::from_vec3(axes.origin + normal.get() * radius),
            normal: Some(normal),
        };
        Ok(Self { axes, radius, poles: [pole(-FRAC_PI_2, axes.normal.reversed()), pole(FRAC_PI_2, axes.normal)] })
    }
}

impl Parametric for Sphere {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        meridian::point(&self.axes, Meridian::Round { centre: 0.0, radius: self.radius }, uv)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        meridian::first(&self.axes, Meridian::Round { centre: 0.0, radius: self.radius }, uv)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        meridian::second(&self.axes, Meridian::Round { centre: 0.0, radius: self.radius }, uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        meridian::derivatives(&self.axes, Meridian::Round { centre: 0.0, radius: self.radius }, uv, order, out);
        Ok(())
    }

    fn domain(&self) -> Domain {
        let latitude = ParamRange { lo: -FRAC_PI_2, hi: FRAC_PI_2, wrap: Wrap::Open, kind: ParamKind::Angle };
        Domain { u: turn(), v: latitude }
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        turned_box(&self.axes, Meridian::Round { centre: 0.0, radius: self.radius }, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let (across, u, height) = cylindrical(&self.axes, p);
        Ok(UvPoint::new(u, atan2(height, across)))
    }

    fn family(&self) -> FamilyName {
        FamilyName("sphere")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push(out);
        out.push(self.radius);
    }

    fn shape(&self) -> Shape {
        let mut values = self.axes.values("centre");
        values.push(NamedValue::length("radius", self.radius));
        Shape { form: "sphere", values }
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        &self.poles
    }

    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        let (centre, radius) = (Point::from_vec3(self.axes.origin), Length::from_millimetres(self.radius));
        Some(Analytic::Sphere { centre, radius })
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_4, PI, TAU};

    use occam_report::Code;
    use occam_units::{Direction, Vec3};

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, slack, tolerance, wrong_form, wrong_second};
    use crate::surface::Surface;

    fn sphere() -> Sphere {
        Sphere::placed(&placement(), Length::from_millimetres(5.0)).unwrap()
    }

    /// A window across the seam at `u` = 0, from pole to pole.
    fn window() -> UvBox {
        UvBox::new(UvPoint::new(-2.0, -FRAC_PI_2), UvPoint::new(4.5, FRAC_PI_2))
    }

    fn along(x: f64, y: f64, z: f64) -> Vec3 {
        local(x, y, z) - local(0.0, 0.0, 0.0)
    }

    #[test]
    fn the_sphere_passes_the_conformance_suite_across_its_seam_and_poles() {
        for window in [window(), UvBox::new(UvPoint::new(0.5, -1.0), UvPoint::new(6.0, 0.3))] {
            let failures = conformance::surface(&sphere(), window);
            assert!(failures.is_empty(), "{window:?}: {failures:#?}");
        }
    }

    #[test]
    fn the_suite_catches_a_sphere_with_a_wrong_second_derivative_or_form() {
        let failed = conformance::checks(&conformance::surface(&wrong_second(sphere()), window()));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
        let Some(Analytic::Sphere { centre, radius }) = Surface::analytic(&sphere(), tolerance()) else {
            panic!("a sphere presents a sphere")
        };
        let larger = Analytic::Sphere { centre, radius: Length::from_millimetres(radius.millimetres() + 1e-3) };
        let failed = conformance::checks(&conformance::surface(&wrong_form(sphere(), larger), window()));
        assert_eq!(failed, [Check::Normals]);
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let s = sphere();
        assert!(agree(s.point(UvPoint::new(0.0, 0.0)).to_vec3(), local(5.0, 0.0, 0.0)));
        assert!(agree(s.point(UvPoint::new(FRAC_PI_2, 0.0)).to_vec3(), local(0.0, 5.0, 0.0)));
        assert!(agree(s.point(UvPoint::new(PI, FRAC_PI_4)).to_vec3(), local(-12.5f64.sqrt(), 0.0, 12.5f64.sqrt())));
        let mut out = vec![];
        Surface::derivatives(&s, UvPoint::new(0.0, 0.0), 3, &mut out, &mut Scratch::default()).unwrap();
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let want = [
            along(0.0, 5.0, 0.0),
            along(0.0, 0.0, 5.0),
            along(-5.0, 0.0, 0.0),
            zero,
            along(-5.0, 0.0, 0.0),
            along(0.0, -5.0, 0.0),
            zero,
            along(0.0, -5.0, 0.0),
            along(0.0, 0.0, -5.0),
        ];
        assert_eq!(out.len(), want.len());
        for (got, want) in out.iter().zip(want) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
    }

    #[test]
    fn its_poles_are_declared_with_their_normals() {
        let s = sphere();
        let up = placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let [south, north] = Surface::degeneracies(&s) else { panic!("two poles") };
        assert_eq!((south.param, south.at, north.param, north.at), (Param::V, -FRAC_PI_2, Param::V, FRAC_PI_2));
        assert_eq!((south.normal, north.normal), (Some(up.reversed()), Some(up)));
        assert!(
            agree(south.point.to_vec3(), local(0.0, 0.0, -5.0)) && agree(north.point.to_vec3(), local(0.0, 0.0, 5.0))
        );
        for u in [0.0, 1.0, 4.0] {
            assert!(agree(s.point(UvPoint::new(u, FRAC_PI_2)).to_vec3(), north.point.to_vec3()));
            assert!(s.first(UvPoint::new(u, -FRAC_PI_2)).du.rate().norm() < 1e-14, "the line collapses");
        }
    }

    #[test]
    fn the_foot_is_the_angles_about_the_centre() {
        let (s, tol) = (sphere(), tolerance());
        let outside = Point::from_vec3(local(3.0, 3.0, 18f64.sqrt()));
        let c = Surface::closest(&s, outside, None, tol, &mut Scratch::default()).unwrap();
        assert!((c.at.u() - FRAC_PI_4).abs() < 1e-14 && (c.at.v() - FRAC_PI_4).abs() < 1e-14, "{c:?}");
        assert!((c.distance.millimetres() - 1.0).abs() < 1e-12, "{c:?}");
        let centre = Point::from_vec3(local(0.0, 0.0, 0.0));
        let any = Surface::closest(&s, centre, None, tol, &mut Scratch::default()).unwrap();
        assert!((any.distance.millimetres() - 5.0).abs() < 1e-12, "every point is as near: {any:?}");
        let above = Surface::closest(&s, Point::from_vec3(local(0.0, 0.0, 9.0)), None, tol, &mut Scratch::default());
        let above = above.unwrap();
        assert!((above.at.v() - FRAC_PI_2).abs() < 1e-14 && (above.distance.millimetres() - 4.0).abs() < 1e-12);
        let behind = Surface::invert(&s, Point::from_vec3(local(-1.0, -1.0, 0.0)), None, tol, &mut Scratch::default());
        assert!((behind.unwrap().u() - 5.0 * FRAC_PI_4).abs() < 1e-14, "the angle is from 0 to 2π");
    }

    #[test]
    fn its_box_is_tight_even_when_tilted() {
        let flat = Sphere::placed(&Placement::identity(), Length::from_millimetres(2.0)).unwrap();
        let near = |got: Vec3, want: [f64; 3], outward: f64| {
            [(got.x, want[0]), (got.y, want[1]), (got.z, want[2])]
                .iter()
                .all(|(g, w)| (g - w) * outward >= 0.0 && (g - w).abs() <= 1e-12)
        };
        for (patch, low, high) in [
            ((0.0, -FRAC_PI_2, TAU, FRAC_PI_2), [-2.0; 3], [2.0; 3]),
            ((0.0, 0.0, FRAC_PI_2, FRAC_PI_2), [0.0; 3], [2.0; 3]),
            ((-FRAC_PI_2, -FRAC_PI_4, FRAC_PI_2, FRAC_PI_4), [0.0, -2.0, -2f64.sqrt()], [2.0, 2.0, 2f64.sqrt()]),
        ] {
            let b = Surface::bounds(&flat, UvBox::new(UvPoint::new(patch.0, patch.1), UvPoint::new(patch.2, patch.3)));
            assert!(near(b.low().to_vec3(), low, -1.0) && near(b.high().to_vec3(), high, 1.0), "{patch:?}: {b:?}");
        }
        let whole = Surface::bounds(&sphere(), window());
        let centre = local(0.0, 0.0, 0.0);
        let (l, h) = (whole.low().to_vec3() - centre, whole.high().to_vec3() - centre);
        assert!(near(l, [-5.0; 3], -1.0) && near(h, [5.0; 3], 1.0), "the whole tilted sphere: {whole:?}");
        let part = UvBox::new(UvPoint::new(-2.0, -1.0), UvPoint::new(1.0, 0.7));
        assert!(slack(&sphere(), part, 200) < 1e-3, "tilted, against samples");
    }

    #[test]
    fn a_radius_must_be_positive() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let refused = Sphere::placed(&placement(), Length::from_millimetres(bad)).unwrap_err();
            assert_eq!(refused.code, Code::SizeNotPositive);
        }
    }

    #[test]
    fn its_shape_content_and_form_name_what_defines_it() {
        let s = sphere();
        let shape = Surface::shape(&s);
        assert_eq!((shape.form, shape.values.len()), ("sphere", 4));
        assert_eq!(shape.values[3], NamedValue::length("radius", 5.0));
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(content.words().len(), 1 + 6 + 12 + 1, "the name, the axes and the radius");
        let form = Surface::analytic(&s, tolerance()).unwrap();
        let centre = Point::from_vec3(local(0.0, 0.0, 0.0));
        assert_eq!(form, Analytic::Sphere { centre, radius: Length::from_millimetres(5.0) });
        assert_eq!(Surface::family(&s), FamilyName("sphere"));
    }
}
