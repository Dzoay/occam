//! Source: original
//! The torus: `centre + (R + r cos v)(cos u x + sin u y) + r sin v z`, where `u` and `v` are angles that both come
//! round: `u` about the axis and `v` about the tube (PARITY 1.5). Its minor radius `r` is less than its major `R`, so
//! it has no degenerate line. Its foot is closed-form: the angle about the axis, then the angle about the tube's centre.

use occam_math::{Real, atan2};
use occam_report::{NamedValue, Quantity, Report};
use occam_units::{Bounds, Length, Local, Placement, Point, Tangent, Tolerance, UvPoint, World};

use crate::analytic::Analytic;
use crate::axes::Axes;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::BsplineSurface;
use crate::domain::{Domain, UvBox};
use crate::exact;
use crate::identity::{Content, FamilyName, Shape};
use crate::meridian::{self, Meridian, cylindrical, turned_box};
use crate::support::{between, positive};
use crate::surface::Parametric;
use crate::turn::{above, turn};

/// A ring torus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Torus {
    axes: Axes,
    major: f64,
    minor: f64,
}

impl Torus {
    /// The torus about the Z axis of `placement`, whose tube of radius `minor` runs round a circle of radius `major`
    /// in its XY plane: `u` from its X axis towards its Y, and `v` about the tube from outside it towards Z.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length greater than zero, and `geom.out-of-range`
    /// for a minor radius not less than the major.
    pub fn placed(placement: &Placement<Local, World>, major: Length, minor: Length) -> Result<Self, Report> {
        let (major, minor) = (positive(major)?, positive(minor)?);
        let minor = between(minor, (0.0, major), Quantity::Length)?;
        Ok(Self { axes: Axes::of(placement), major, minor })
    }
}

impl Parametric for Torus {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        meridian::point(&self.axes, Meridian::Round { centre: self.major, radius: self.minor }, uv)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        meridian::first(&self.axes, Meridian::Round { centre: self.major, radius: self.minor }, uv)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        meridian::second(&self.axes, Meridian::Round { centre: self.major, radius: self.minor }, uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        meridian::derivatives(&self.axes, Meridian::Round { centre: self.major, radius: self.minor }, uv, order, out);
        Ok(())
    }

    fn domain(&self) -> Domain {
        Domain { u: turn(), v: turn() }
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        turned_box(&self.axes, Meridian::Round { centre: self.major, radius: self.minor }, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let (across, u, height) = cylindrical(&self.axes, p);
        Ok(UvPoint::new(u, above(atan2(height, across - self.major), 0.0)))
    }

    fn family(&self) -> FamilyName {
        FamilyName("torus")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push(out);
        out.push(self.major);
        out.push(self.minor);
    }

    fn shape(&self) -> Shape {
        let mut values = self.axes.values("centre");
        values.push(NamedValue::length("major-radius", self.major));
        values.push(NamedValue::length("minor-radius", self.minor));
        Shape { form: "torus", values }
    }

    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        let (major, minor) = (Length::from_millimetres(self.major), Length::from_millimetres(self.minor));
        Some(Analytic::Torus { centre: Point::from_vec3(self.axes.origin), axis: self.axes.normal, major, minor })
    }

    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        exact::turned(
            &self.axes,
            Meridian::Round { centre: self.major, radius: self.minor },
            Parametric::domain(self),
            patch,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use occam_report::Code;
    use occam_units::{Direction, Vec3};

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, slack, tolerance, wrong_form, wrong_second};
    use crate::surface::Surface;

    fn torus() -> Torus {
        Torus::placed(&placement(), Length::from_millimetres(8.0), Length::from_millimetres(3.0)).unwrap()
    }

    /// A window across both seams.
    fn window() -> UvBox {
        UvBox::new(UvPoint::new(-1.0, -2.0), UvPoint::new(5.0, 3.0))
    }

    fn along(x: f64, y: f64, z: f64) -> Vec3 {
        local(x, y, z) - local(0.0, 0.0, 0.0)
    }

    #[test]
    fn the_torus_passes_the_conformance_suite_across_both_seams() {
        for window in [window(), UvBox::new(UvPoint::new(0.5, 0.5), UvPoint::new(6.0, 6.0))] {
            let failures = conformance::surface(&torus(), window);
            assert!(failures.is_empty(), "{window:?}: {failures:#?}");
        }
    }

    #[test]
    fn the_suite_catches_a_torus_with_a_wrong_second_derivative_or_form() {
        let failed = conformance::checks(&conformance::surface(&wrong_second(torus()), window()));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
        let Some(Analytic::Torus { centre, axis, major, minor }) = Surface::analytic(&torus(), tolerance()) else {
            panic!("a torus presents a torus")
        };
        let fatter =
            Analytic::Torus { centre, axis, major, minor: Length::from_millimetres(minor.millimetres() + 1e-3) };
        let failed = conformance::checks(&conformance::surface(&wrong_form(torus(), fatter), window()));
        assert_eq!(failed, [Check::Normals]);
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let s = torus();
        assert!(agree(s.point(UvPoint::new(0.0, 0.0)).to_vec3(), local(11.0, 0.0, 0.0)));
        assert!(agree(s.point(UvPoint::new(0.0, PI)).to_vec3(), local(5.0, 0.0, 0.0)));
        assert!(agree(s.point(UvPoint::new(FRAC_PI_2, FRAC_PI_2)).to_vec3(), local(0.0, 8.0, 3.0)));
        let mut out = vec![];
        Surface::derivatives(&s, UvPoint::new(0.0, 0.0), 3, &mut out, &mut Scratch::default()).unwrap();
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let want = [
            along(0.0, 11.0, 0.0),
            along(0.0, 0.0, 3.0),
            along(-11.0, 0.0, 0.0),
            zero,
            along(-3.0, 0.0, 0.0),
            along(0.0, -11.0, 0.0),
            zero,
            along(0.0, -3.0, 0.0),
            along(0.0, 0.0, -3.0),
        ];
        assert_eq!(out.len(), want.len());
        for (got, want) in out.iter().zip(want) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
    }

    #[test]
    fn the_foot_is_the_angle_about_the_axis_then_about_the_tube() {
        let (s, tol) = (torus(), tolerance());
        let foot = |x: f64, z: f64| {
            Surface::closest(&s, Point::from_vec3(local(x, 0.0, z)), None, tol, &mut Scratch::default()).unwrap()
        };
        let outside = foot(15.0, 4.0);
        let (sin_u, cos_u) = occam_math::sin_cos(outside.at.u());
        assert!(sin_u.abs() < 1e-14 && cos_u > 0.0, "u is 0 or 2π, the seam: {outside:?}");
        assert!((outside.at.v() - atan2(4.0, 7.0)).abs() < 1e-14, "{outside:?}");
        assert!((outside.distance.millimetres() - (65f64.sqrt() - 3.0)).abs() < 1e-12, "{outside:?}");
        let on_axis = foot(0.0, 4.0);
        assert!((on_axis.distance.millimetres() - (80f64.sqrt() - 3.0)).abs() < 1e-12, "{on_axis:?}");
        let centre = foot(0.0, 0.0);
        assert!((centre.at.v() - PI).abs() < 1e-14 && (centre.distance.millimetres() - 5.0).abs() < 1e-12);
        let tube = foot(8.0, 0.0);
        assert!((tube.distance.millimetres() - 3.0).abs() < 1e-12, "every point of the tube is as near: {tube:?}");
        let below = foot(9.0, -1.0);
        assert!((below.at.v() - (TAU - std::f64::consts::FRAC_PI_4)).abs() < 1e-14, "v is from 0 to 2π: {below:?}");
    }

    #[test]
    fn its_box_is_tight_even_when_tilted() {
        let flat = Torus::placed(&Placement::identity(), Length::from_millimetres(4.0), Length::from_millimetres(1.0));
        let flat = flat.unwrap();
        let near = |got: Vec3, want: [f64; 3], outward: f64| {
            [(got.x, want[0]), (got.y, want[1]), (got.z, want[2])]
                .iter()
                .all(|(g, w)| (g - w) * outward >= 0.0 && (g - w).abs() <= 1e-12)
        };
        for (patch, low, high) in [
            ((0.0, 0.0, TAU, TAU), [-5.0, -5.0, -1.0], [5.0, 5.0, 1.0]),
            ((0.0, 0.0, FRAC_PI_2, TAU), [0.0, 0.0, -1.0], [5.0, 5.0, 1.0]),
            ((0.0, FRAC_PI_2, TAU, 1.5 * PI), [-4.0, -4.0, -1.0], [4.0, 4.0, 1.0]),
            // Ending just past the top of the tube, and starting just short of its bottom.
            ((0.0, 0.0, TAU, 1.7), [-5.0, -5.0, 0.0], [5.0, 5.0, 1.0]),
            ((0.0, -1.7, TAU, 0.0), [-5.0, -5.0, -1.0], [5.0, 5.0, 0.0]),
        ] {
            let b = Surface::bounds(&flat, UvBox::new(UvPoint::new(patch.0, patch.1), UvPoint::new(patch.2, patch.3)));
            assert!(near(b.low().to_vec3(), low, -1.0) && near(b.high().to_vec3(), high, 1.0), "{patch:?}: {b:?}");
        }
        let whole = Surface::bounds(&torus(), UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(TAU, TAU)));
        let axis = placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).get();
        let reach = |n: f64| 8.0 * (1.0 - n * n).sqrt() + 3.0;
        let centre = local(0.0, 0.0, 0.0);
        let (l, h) = (whole.low().to_vec3() - centre, whole.high().to_vec3() - centre);
        let want = [reach(axis.x), reach(axis.y), reach(axis.z)];
        assert!(near(l, want.map(|r| -r), -1.0) && near(h, want, 1.0), "the whole tilted torus: {whole:?}");
        assert!(slack(&torus(), window(), 200) < 1e-3, "tilted, against samples");
    }

    #[test]
    fn its_radii_must_be_positive_and_the_minor_less_than_the_major() {
        let torus_of = |major: f64, minor: f64| {
            Torus::placed(&placement(), Length::from_millimetres(major), Length::from_millimetres(minor))
        };
        for (major, minor) in [(0.0, 1.0), (5.0, 0.0), (-5.0, 1.0), (5.0, f64::NAN)] {
            assert_eq!(torus_of(major, minor).unwrap_err().code, Code::SizeNotPositive, "{major} {minor}");
        }
        for (major, minor) in [(5.0, 5.0), (5.0, 6.0)] {
            assert_eq!(torus_of(major, minor).unwrap_err().code, Code::OutOfRange, "{major} {minor}");
        }
    }

    #[test]
    fn its_shape_content_and_form_name_what_defines_it() {
        let s = torus();
        let shape = Surface::shape(&s);
        assert_eq!((shape.form, shape.values.len()), ("torus", 5));
        assert_eq!(shape.values[3], NamedValue::length("major-radius", 8.0));
        assert_eq!(shape.values[4], NamedValue::length("minor-radius", 3.0));
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(content.words().len(), 1 + 5 + 12 + 2, "the name, the axes and the radii");
        assert_eq!(content.words().last(), Some(&3f64.to_bits()));
        let form = Surface::analytic(&s, tolerance()).unwrap();
        let axis = placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let (major, minor) = (Length::from_millimetres(8.0), Length::from_millimetres(3.0));
        assert_eq!(form, Analytic::Torus { centre: Point::from_vec3(local(0.0, 0.0, 0.0)), axis, major, minor });
        assert_eq!((Surface::family(&s), Surface::degeneracies(&s)), (FamilyName("torus"), &[][..]));
    }
}
