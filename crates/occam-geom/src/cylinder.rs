//! Source: original
//! The cylinder: `origin + r cos u x + r sin u y + v z`, where `u` is an angle that comes round and `v` a length along
//! the axis (PARITY 1.5). Its foot is closed-form: the angle about the axis and the height along it.

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Local, ParamKind, Placement, Point, Tangent, Tolerance, UvPoint, World};

use crate::analytic::Analytic;
use crate::axes::Axes;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::BsplineSurface;
use crate::domain::{Domain, ParamRange, UvBox};
use crate::exact;
use crate::identity::{Content, FamilyName, Shape};
use crate::meridian::{self, Meridian, cylindrical, turned_box};
use crate::support::positive;
use crate::surface::Parametric;
use crate::turn::turn;

/// A cylinder, unbounded along its axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylinder {
    axes: Axes,
    radius: f64,
}

impl Cylinder {
    /// The cylinder of `radius` about the Z axis of `placement`: `u` from its X axis towards its Y, and `v` from its
    /// origin along its Z.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length greater than zero.
    pub fn placed(placement: &Placement<Local, World>, radius: Length) -> Result<Self, Report> {
        Ok(Self { axes: Axes::of(placement), radius: positive(radius)? })
    }
}

impl Parametric for Cylinder {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        meridian::point(&self.axes, Meridian::Straight { start: self.radius, slope: (0.0, 1.0) }, uv)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        meridian::first(&self.axes, Meridian::Straight { start: self.radius, slope: (0.0, 1.0) }, uv)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        meridian::second(&self.axes, Meridian::Straight { start: self.radius, slope: (0.0, 1.0) }, uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        meridian::derivatives(&self.axes, Meridian::Straight { start: self.radius, slope: (0.0, 1.0) }, uv, order, out);
        Ok(())
    }

    fn domain(&self) -> Domain {
        Domain { u: turn(), v: ParamRange::unbounded(ParamKind::Length) }
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        turned_box(&self.axes, Meridian::Straight { start: self.radius, slope: (0.0, 1.0) }, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let (_, u, height) = cylindrical(&self.axes, p);
        Ok(UvPoint::new(u, height))
    }

    fn family(&self) -> FamilyName {
        FamilyName("cylinder")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push(out);
        out.push(self.radius);
    }

    fn shape(&self) -> Shape {
        let mut values = self.axes.values("origin");
        values.push(NamedValue::length("radius", self.radius));
        Shape { form: "cylinder", values }
    }

    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        let (on_axis, radius) = (Point::from_vec3(self.axes.origin), Length::from_millimetres(self.radius));
        Some(Analytic::Cylinder { on_axis, axis: self.axes.normal, radius })
    }

    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        exact::turned(
            &self.axes,
            Meridian::Straight { start: self.radius, slope: (0.0, 1.0) },
            Parametric::domain(self),
            patch,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    use occam_report::Code;
    use occam_units::Vec3;

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, slack, tolerance, wrong_form, wrong_second};
    use crate::surface::Surface;

    fn cylinder() -> Cylinder {
        Cylinder::placed(&placement(), Length::from_millimetres(6.0)).unwrap()
    }

    /// A window across the seam at `u` = 0.
    fn window() -> UvBox {
        UvBox::new(UvPoint::new(-1.0, -15.0), UvPoint::new(2.5, 25.0))
    }

    fn along(x: f64, y: f64, z: f64) -> Vec3 {
        local(x, y, z) - local(0.0, 0.0, 0.0)
    }

    #[test]
    fn the_cylinder_passes_the_conformance_suite_across_its_seam() {
        for window in [window(), UvBox::new(UvPoint::new(0.5, 1.0), UvPoint::new(6.0, 2.0))] {
            let failures = conformance::surface(&cylinder(), window);
            assert!(failures.is_empty(), "{window:?}: {failures:#?}");
        }
    }

    #[test]
    fn the_suite_catches_a_cylinder_with_a_wrong_second_derivative_or_form() {
        let failed = conformance::checks(&conformance::surface(&wrong_second(cylinder()), window()));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
        let Some(Analytic::Cylinder { on_axis, axis, radius }) = Surface::analytic(&cylinder(), tolerance()) else {
            panic!("a cylinder presents a cylinder")
        };
        let wider = Analytic::Cylinder { on_axis, axis, radius: Length::from_millimetres(radius.millimetres() + 1e-3) };
        let failed = conformance::checks(&conformance::surface(&wrong_form(cylinder(), wider), window()));
        assert_eq!(failed, [Check::Normals]);
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let s = cylinder();
        assert!(agree(s.point(UvPoint::new(0.0, 0.0)).to_vec3(), local(6.0, 0.0, 0.0)));
        assert!(agree(s.point(UvPoint::new(FRAC_PI_2, 3.0)).to_vec3(), local(0.0, 6.0, 3.0)));
        assert!(agree(s.point(UvPoint::new(PI, -2.0)).to_vec3(), local(-6.0, 0.0, -2.0)));
        let mut out = vec![];
        Surface::derivatives(&s, UvPoint::new(0.0, 1.0), 3, &mut out, &mut Scratch::default()).unwrap();
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let want = [
            along(0.0, 6.0, 0.0),
            along(0.0, 0.0, 1.0),
            along(-6.0, 0.0, 0.0),
            zero,
            zero,
            along(0.0, -6.0, 0.0),
            zero,
            zero,
            zero,
        ];
        assert_eq!(out.len(), want.len());
        for (got, want) in out.iter().zip(want) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
    }

    #[test]
    fn the_foot_is_the_angle_about_the_axis_and_the_height_along_it() {
        let (s, tol) = (cylinder(), tolerance());
        let outside = Point::from_vec3(local(9.0, 9.0, 4.0));
        let c = Surface::closest(&s, outside, None, tol, &mut Scratch::default()).unwrap();
        assert!((c.at.u() - FRAC_PI_4).abs() < 1e-14 && (c.at.v() - 4.0).abs() < 1e-12, "{c:?}");
        assert!((c.distance.millimetres() - (162f64.sqrt() - 6.0)).abs() < 1e-12, "{c:?}");
        let behind = Point::from_vec3(local(-1.0, -1.0, -3.0));
        let below = Surface::invert(&s, behind, None, tol, &mut Scratch::default()).unwrap();
        assert!((below.u() - 5.0 * FRAC_PI_4).abs() < 1e-14, "{below:?}: the angle is from 0 to 2π");
        let on_axis = Point::from_vec3(local(0.0, 0.0, 7.0));
        let any = Surface::closest(&s, on_axis, None, tol, &mut Scratch::default()).unwrap();
        assert!((any.distance.millimetres() - 6.0).abs() < 1e-12 && (any.at.v() - 7.0).abs() < 1e-12, "{any:?}");
    }

    #[test]
    fn its_box_reaches_the_arcs_at_its_ends_and_no_further() {
        let flat = Cylinder::placed(&Placement::identity(), Length::from_millimetres(2.0)).unwrap();
        for (patch, low, high) in [
            ((0.0, -1.0, FRAC_PI_2, 3.0), [0.0, 0.0, -1.0], [2.0, 2.0, 3.0]),
            ((0.0, 0.0, 2.0 * PI, 1.0), [-2.0, -2.0, 0.0], [2.0, 2.0, 1.0]),
            ((-FRAC_PI_2, 0.0, FRAC_PI_2, 1.0), [0.0, -2.0, 0.0], [2.0, 2.0, 1.0]),
        ] {
            let b = Surface::bounds(&flat, UvBox::new(UvPoint::new(patch.0, patch.1), UvPoint::new(patch.2, patch.3)));
            let (l, h) = (b.low().to_vec3(), b.high().to_vec3());
            for (got, want) in [(l.x, low[0]), (l.y, low[1]), (l.z, low[2])] {
                assert!(got <= want && got >= want - 1e-12, "{patch:?}: {b:?}");
            }
            for (got, want) in [(h.x, high[0]), (h.y, high[1]), (h.z, high[2])] {
                assert!(got >= want && got <= want + 1e-12, "{patch:?}: {b:?}");
            }
        }
        assert!(slack(&cylinder(), window(), 200) < 1e-3, "tilted, against samples");
        let unbounded = UvBox::new(UvPoint::new(0.0, f64::NEG_INFINITY), UvPoint::new(1.0, 0.0));
        assert!(Surface::bounds(&flat, unbounded).contains(Point::from_vec3(Vec3::new(1e300, -1e300, 1e300))));
    }

    #[test]
    fn a_radius_must_be_positive() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let refused = Cylinder::placed(&placement(), Length::from_millimetres(bad)).unwrap_err();
            assert_eq!(refused.code, Code::SizeNotPositive);
        }
    }

    #[test]
    fn its_shape_content_and_form_name_what_defines_it() {
        let s = cylinder();
        let shape = Surface::shape(&s);
        assert_eq!((shape.form, shape.values.len()), ("cylinder", 4));
        assert_eq!(shape.values[3], NamedValue::length("radius", 6.0));
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(content.words().len(), 1 + 8 + 12 + 1, "the name, the axes and the radius");
        assert_eq!(content.words().last(), Some(&6f64.to_bits()));
        let form = Surface::analytic(&s, tolerance()).unwrap();
        let axis = placement().direction(occam_units::Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let on_axis = Point::from_vec3(local(0.0, 0.0, 0.0));
        assert_eq!(form, Analytic::Cylinder { on_axis, axis, radius: Length::from_millimetres(6.0) });
        assert_eq!((Surface::family(&s), Surface::degeneracies(&s)), (FamilyName("cylinder"), &[][..]));
    }
}
