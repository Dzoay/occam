//! Source: original
//! The cone: `origin + (r + v sin α)(cos u x + sin u y) + v cos α z`, where `u` is an angle that comes round and `v` a
//! length along the surface from the circle of radius `r` (PARITY 1.5). It is unbounded both ways, through its apex,
//! where `v` is `−r / sin α`, into its other nappe. Its foot is closed-form: the nearer of the feet on the two lines in
//! which the plane through the axis and the point cuts it.

use std::f64::consts::FRAC_PI_2;

use occam_math::{Real, sin_cos};
use occam_report::{NamedValue, Quantity, Report};
use occam_units::{Angle, Bounds, Length, Local, ParamKind, Placement, Point, Tangent, Tolerance, UvPoint, World};

use crate::analytic::Analytic;
use crate::axes::Axes;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::BsplineSurface;
use crate::domain::{Degeneracy, Domain, Param, ParamRange, UvBox};
use crate::exact;
use crate::identity::{Content, FamilyName, Shape};
use crate::meridian::{self, Meridian, cylindrical, opposite, turned_box};
use crate::support::{between, not_negative};
use crate::surface::Parametric;
use crate::turn::turn;

/// A cone, both nappes, unbounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    axes: Axes,
    radius: f64,
    half_angle: f64,
    /// The sine and cosine of the half-angle, computed once (S4's cone).
    slope: (f64, f64),
    /// The apex, where the line of constant `v` collapses to a point with no normal.
    apex: [Degeneracy; 1],
}

impl Cone {
    /// The cone through the circle of `radius` about the origin of `placement`, in its XY plane, opening along its Z at
    /// `half_angle` from the axis: `u` from its X axis towards its Y, and `v` from that circle along the surface, rising
    /// along Z. A radius of zero puts the apex at the origin.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length of zero or more, and `geom.out-of-range` for
    /// a half-angle not strictly between zero and a right angle.
    pub fn placed(placement: &Placement<Local, World>, radius: Length, half_angle: Angle) -> Result<Self, Report> {
        let radius = not_negative(radius)?;
        let half_angle = between(half_angle.radians(), (0.0, FRAC_PI_2), Quantity::Angle)?;
        let (axes, slope) = (Axes::of(placement), sin_cos(half_angle));
        let at = -radius / slope.0;
        let point = Point::from_vec3(axes.origin + axes.normal.get() * (at * slope.1));
        Ok(Self { axes, radius, half_angle, slope, apex: [Degeneracy { param: Param::V, at, point, normal: None }] })
    }
}

impl Parametric for Cone {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        meridian::point(&self.axes, Meridian::Straight { start: self.radius, slope: self.slope }, uv)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        meridian::first(&self.axes, Meridian::Straight { start: self.radius, slope: self.slope }, uv)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        meridian::second(&self.axes, Meridian::Straight { start: self.radius, slope: self.slope }, uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        meridian::derivatives(&self.axes, Meridian::Straight { start: self.radius, slope: self.slope }, uv, order, out);
        Ok(())
    }

    fn domain(&self) -> Domain {
        Domain { u: turn(), v: ParamRange::unbounded(ParamKind::Length) }
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        turned_box(&self.axes, Meridian::Straight { start: self.radius, slope: self.slope }, patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        // In the plane through the axis and `p`, `p` being `(across, height)`, the cone is the line at `u`, through
        // `(r, 0)` along `(sin α, cos α)`, and its mirror across the axis, the line at the opposite angle. The foot is on
        // the nearer, `v` along it.
        let (across, u, height) = cylindrical(&self.axes, p);
        let (sin, cos) = self.slope;
        let near_gap = ((across - self.radius) * cos - height * sin).abs();
        let far_gap = ((across + self.radius) * cos + height * sin).abs();
        Ok(if far_gap < near_gap {
            UvPoint::new(opposite(&self.axes, p), height * cos - (across + self.radius) * sin)
        } else {
            UvPoint::new(u, (across - self.radius) * sin + height * cos)
        })
    }

    fn family(&self) -> FamilyName {
        FamilyName("cone")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push(out);
        out.push(self.radius);
        out.push(self.half_angle);
    }

    fn shape(&self) -> Shape {
        let mut values = self.axes.values("origin");
        values.push(NamedValue::length("radius", self.radius));
        values.push(NamedValue::new("half-angle", Quantity::Angle, self.half_angle));
        Shape { form: "cone", values }
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        &self.apex
    }

    fn analytic(&self, _tol: Tolerance) -> Option<Analytic> {
        let [apex] = self.apex;
        let half_angle = Angle::from_radians(self.half_angle);
        Some(Analytic::Cone { apex: apex.point, axis: self.axes.normal, half_angle })
    }

    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        exact::turned(
            &self.axes,
            Meridian::Straight { start: self.radius, slope: self.slope },
            Parametric::domain(self),
            patch,
        )
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

    fn cone() -> Cone {
        Cone::placed(&placement(), Length::from_millimetres(4.0), Angle::from_radians(0.5)).unwrap()
    }

    /// The apex's `v`.
    fn apex() -> f64 {
        -4.0 / sin_cos(0.5).0
    }

    /// A window across the seam at `u` = 0 and through the apex, into both nappes.
    fn window() -> UvBox {
        UvBox::new(UvPoint::new(-1.0, -20.0), UvPoint::new(2.5, 10.0))
    }

    fn along(x: f64, y: f64, z: f64) -> Vec3 {
        local(x, y, z) - local(0.0, 0.0, 0.0)
    }

    #[test]
    fn the_cone_passes_the_conformance_suite_across_its_seam_and_apex() {
        let pointed = Cone::placed(&placement(), Length::from_millimetres(0.0), Angle::from_radians(1.2)).unwrap();
        for (s, window) in [
            (cone(), window()),
            (cone(), UvBox::new(UvPoint::new(0.5, 1.0), UvPoint::new(6.0, 3.0))),
            (pointed, UvBox::new(UvPoint::new(-3.0, -2.0), UvPoint::new(3.0, 5.0))),
        ] {
            let failures = conformance::surface(&s, window);
            assert!(failures.is_empty(), "{window:?}: {failures:#?}");
        }
    }

    #[test]
    fn the_suite_catches_a_cone_with_a_wrong_second_derivative_or_form() {
        let failed = conformance::checks(&conformance::surface(&wrong_second(cone()), window()));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
        let Some(Analytic::Cone { apex, axis, half_angle }) = Surface::analytic(&cone(), tolerance()) else {
            panic!("a cone presents a cone")
        };
        let wider = Analytic::Cone { apex, axis, half_angle: Angle::from_radians(half_angle.radians() + 1e-4) };
        let failed = conformance::checks(&conformance::surface(&wrong_form(cone(), wider), window()));
        assert_eq!(failed, [Check::Normals]);
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let (s, (sin, cos)) = (cone(), sin_cos(0.5));
        assert!(agree(s.point(UvPoint::new(0.0, 0.0)).to_vec3(), local(4.0, 0.0, 0.0)));
        assert!(agree(s.point(UvPoint::new(FRAC_PI_2, 2.0)).to_vec3(), local(0.0, 4.0 + 2.0 * sin, 2.0 * cos)));
        assert!(agree(s.point(UvPoint::new(0.0, apex())).to_vec3(), local(0.0, 0.0, apex() * cos)));
        let mut out = vec![];
        Surface::derivatives(&s, UvPoint::new(0.0, 1.0), 3, &mut out, &mut Scratch::default()).unwrap();
        let (zero, rho) = (Vec3::new(0.0, 0.0, 0.0), 4.0 + sin);
        let want = [
            along(0.0, rho, 0.0),
            along(sin, 0.0, cos),
            along(-rho, 0.0, 0.0),
            along(0.0, sin, 0.0),
            zero,
            along(0.0, -rho, 0.0),
            along(-sin, 0.0, 0.0),
            zero,
            zero,
        ];
        assert_eq!(out.len(), want.len());
        for (got, want) in out.iter().zip(want) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
    }

    #[test]
    fn its_apex_is_declared_with_no_normal() {
        let s = cone();
        let apex_point = local(0.0, 0.0, apex() * sin_cos(0.5).1);
        let [line] = Surface::degeneracies(&s) else { panic!("one apex") };
        assert_eq!((line.param, line.at, line.normal), (Param::V, apex(), None));
        assert!(agree(line.point.to_vec3(), apex_point));
        let jet = s.first(UvPoint::new(1.0, apex()));
        assert!(jet.du.rate().norm() < 1e-14, "the line collapses: {jet:?}");
        let pointed = Cone::placed(&placement(), Length::from_millimetres(0.0), Angle::from_radians(1.0)).unwrap();
        let [tip] = Surface::degeneracies(&pointed) else { panic!("one apex") };
        assert_eq!(tip.at.to_bits(), (-0.0f64).to_bits(), "a radius of zero puts the apex where v is 0");
        assert!(agree(tip.point.to_vec3(), local(0.0, 0.0, 0.0)));
    }

    #[test]
    fn the_foot_is_on_the_nearer_line_through_the_axis() {
        let (s, tol, (sin, cos)) = (cone(), tolerance(), sin_cos(0.5));
        let apex_height = apex() * cos;
        let foot = |x: f64, z: f64| {
            let c = Surface::closest(&s, Point::from_vec3(local(x, 0.0, z)), None, tol, &mut Scratch::default());
            c.unwrap()
        };
        let outside = foot(10.0, 3.0);
        let (sin_u, cos_u) = sin_cos(outside.at.u());
        assert!(sin_u.abs() < 1e-14 && cos_u > 0.0, "u is 0 or 2π, the seam: {outside:?}");
        assert!((outside.distance.millimetres() - (6.0 * cos - 3.0 * sin)).abs() < 1e-12, "{outside:?}");
        let beyond = foot(1.0, -15.0);
        let to_mirror = (cos + (-15.0 - apex_height) * sin).abs();
        assert!((beyond.at.u() - PI).abs() < 1e-14, "the other nappe's line is nearer: {beyond:?}");
        assert!((beyond.distance.millimetres() - to_mirror).abs() < 1e-12, "{beyond:?} {to_mirror}");
        let tip = foot(0.0, apex_height);
        assert!((tip.at.v() - apex()).abs() < 1e-12 && tip.distance.millimetres() < 1e-12, "{tip:?}");
        let on_axis = foot(0.0, 5.0);
        assert!((on_axis.distance.millimetres() - (5.0 - apex_height) * sin).abs() < 1e-12, "{on_axis:?}");
        let behind = Surface::invert(&s, Point::from_vec3(local(-3.0, -3.0, 1.0)), None, tol, &mut Scratch::default());
        assert!((behind.unwrap().u() - 5.0 * FRAC_PI_4).abs() < 1e-14, "the angle is from 0 to 2π");
        // Beyond the apex at angles 1 and 4 about the axis: the far line is half a turn round, from 0 to 2π.
        for (angle, far) in [(1.0, 1.0 + PI), (4.0, 4.0 - PI)] {
            let (sin_a, cos_a) = sin_cos(angle);
            let p = Point::from_vec3(local(cos_a, sin_a, -15.0));
            let at = Surface::invert(&s, p, None, tol, &mut Scratch::default()).unwrap();
            assert!((at.u() - far).abs() < 1e-14 && (at.v() - (-15.0 * cos - 5.0 * sin)).abs() < 1e-12, "{at:?}");
        }
    }

    #[test]
    fn the_foot_of_every_point_about_the_apex_is_on_the_nearer_line() {
        // A grid in the cone's XZ plane, both sides of the axis and both nappes, against its two lines in that plane
        // sampled every hundredth of a millimetre.
        let (s, tol) = (cone(), tolerance());
        let lines: Vec<Vec3> = (-4000..=2000)
            .flat_map(|k| [0.0, PI].map(|u| s.point(UvPoint::new(u, f64::from(k) / 100.0)).to_vec3()))
            .collect();
        for i in -12..=12 {
            for j in -12..=6 {
                let p = local(f64::from(i), 0.0, apex() * sin_cos(0.5).1 + f64::from(j));
                let sampled =
                    lines.iter().map(|q| (*q - p).norm()).fold(f64::INFINITY, |a, d| if d < a { d } else { a });
                let c = Surface::closest(&s, Point::from_vec3(p), None, tol, &mut Scratch::default()).unwrap();
                assert!(c.distance.millimetres() <= sampled + 1e-9, "({i}, {j}): {c:?} against {sampled}");
            }
        }
    }

    #[test]
    fn its_box_reaches_the_arcs_at_its_ends_and_no_further() {
        let flat = Cone::placed(&Placement::identity(), Length::from_millimetres(2.0), Angle::from_radians(FRAC_PI_4));
        let flat = flat.unwrap();
        let root = 2f64.sqrt();
        for (patch, low, high) in [
            ((0.0, 0.0, FRAC_PI_2, root), [0.0, 0.0, 0.0], [3.0, 3.0, 1.0]),
            ((0.0, -4.0 * root, TAU, 0.0), [-2.0, -2.0, -4.0], [2.0, 2.0, 0.0]),
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
        assert!(slack(&cone(), window(), 200) < 1e-3, "tilted and through the apex, against samples");
    }

    #[test]
    fn a_radius_must_not_be_negative_nor_the_half_angle_outside_a_right_angle() {
        let angle = Angle::from_radians(0.5);
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            let refused = Cone::placed(&placement(), Length::from_millimetres(bad), angle).unwrap_err();
            assert_eq!(refused.code, Code::SizeNotPositive);
        }
        for bad in [0.0, FRAC_PI_2, -0.3, 2.0, f64::NAN] {
            let refused = Cone::placed(&placement(), Length::from_millimetres(1.0), Angle::from_radians(bad));
            assert_eq!(refused.unwrap_err().code, Code::OutOfRange, "{bad}");
        }
        let content = |radius: f64| {
            let s = Cone::placed(&placement(), Length::from_millimetres(radius), angle).unwrap();
            let mut out = Content::default();
            Surface::content(&s, &mut out);
            out
        };
        assert_eq!(content(-0.0), content(0.0), "a radius of zero has one sign");
    }

    #[test]
    fn its_shape_content_and_form_name_what_defines_it() {
        let s = cone();
        let shape = Surface::shape(&s);
        assert_eq!((shape.form, shape.values.len()), ("cone", 5));
        assert_eq!(shape.values[3], NamedValue::length("radius", 4.0));
        assert_eq!(shape.values[4], NamedValue::new("half-angle", Quantity::Angle, 0.5));
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(content.words().len(), 1 + 4 + 12 + 2, "the name, the axes, the radius and the half-angle");
        assert_eq!(content.words().last(), Some(&0.5f64.to_bits()));
        let axis = placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let Some(Analytic::Cone { apex: tip, axis: got, half_angle }) = Surface::analytic(&s, tolerance()) else {
            panic!("a cone presents a cone")
        };
        assert!(agree(tip.to_vec3(), local(0.0, 0.0, apex() * sin_cos(0.5).1)));
        assert_eq!((got, half_angle.radians(), Surface::family(&s)), (axis, 0.5, FamilyName("cone")));
    }
}
