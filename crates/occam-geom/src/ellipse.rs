//! Source: original
//! The ellipse: `centre + a cos t x + b sin t y`, where `t` is an angle that comes round, whole or trimmed to an arc
//! (PARITY 2.1). Its foot has no closed form, so it is searched for.

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Local, Placement, Point, Tangent, World};

use crate::axes::Axes;
use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::exact::{self, Conic};
use crate::identity::{Content, FamilyName, Shape};
use crate::support::positive;
use crate::turn::{arc_box, on_turn, on_turn_first, on_turn_second, turn, turn_derivatives};

/// An ellipse, or an arc of one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    axes: Axes,
    radii: (f64, f64),
    range: ParamRange,
}

impl Ellipse {
    /// The ellipse about the origin of `placement`, in its XY plane, reaching `x_radius` along its X axis and
    /// `y_radius` along its Y, from its X axis towards its Y.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length greater than zero.
    pub fn placed(placement: &Placement<Local, World>, x_radius: Length, y_radius: Length) -> Result<Self, Report> {
        Ok(Self { axes: Axes::of(placement), radii: (positive(x_radius)?, positive(y_radius)?), range: turn() })
    }

    /// The arc from angle `lo` to `hi`.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`].
    pub fn trimmed(self, lo: f64, hi: f64) -> Result<Self, Report> {
        Ok(Self { range: self.range.trimmed(lo, hi)?, ..self })
    }
}

impl ParametricCurve for Ellipse {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        on_turn(&self.axes, self.radii, t)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        on_turn_first(&self.axes, self.radii, t)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        on_turn_second(&self.axes, self.radii, t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        turn_derivatives(&self.axes, self.radii, t, order, out);
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let ends = (self.eval_point(lo).to_vec3(), self.eval_point(hi).to_vec3());
        arc_box(&self.axes, self.radii, (lo, hi), ends)
    }

    fn family(&self) -> FamilyName {
        FamilyName("ellipse")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push_curve(out, &[self.radii.0, self.radii.1], self.range);
    }

    fn shape(&self) -> Shape {
        Shape {
            form: "ellipse",
            values: self.axes.curve_values(
                "centre",
                &[NamedValue::length("x-radius", self.radii.0), NamedValue::length("y-radius", self.radii.1)],
                self.range,
            ),
        }
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        exact::conic_curve(Conic::Elliptic, (&self.axes, self.radii), self.range, (lo, hi))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use occam_report::{Code, Quantity};

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, tolerance};
    use crate::curve::Curve;

    fn ellipse() -> Ellipse {
        Ellipse::placed(&placement(), Length::from_millimetres(12.0), Length::from_millimetres(5.0)).unwrap()
    }

    #[test]
    fn the_ellipse_and_its_arcs_pass_the_conformance_suite() {
        for (lo, hi, whole) in [(0.0, TAU, true), (0.5, 2.5, false), (5.0, 7.0, false)] {
            let e = if whole { ellipse() } else { ellipse().trimmed(lo, hi).unwrap() };
            let failures = conformance::curve(&e, lo, hi);
            assert!(failures.is_empty(), "{lo} {hi}: {failures:#?}");
        }
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let e = ellipse();
        assert!(agree(e.point(0.0).to_vec3(), local(12.0, 0.0, 0.0)));
        assert!(agree(e.point(FRAC_PI_2).to_vec3(), local(0.0, 5.0, 0.0)));
        assert!(agree(e.point(PI).to_vec3(), local(-12.0, 0.0, 0.0)));
        let along = |x: f64, y: f64| local(x, y, 0.0) - local(0.0, 0.0, 0.0);
        let mut out = vec![];
        Curve::derivatives(&e, 0.0, 4, &mut out, &mut Scratch::default()).unwrap();
        for (got, want) in out.iter().zip([along(0.0, 5.0), along(-12.0, 0.0), along(0.0, -5.0), along(12.0, 0.0)]) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
        assert_eq!(e.first(0.0).1, out[0]);
    }

    #[test]
    fn the_foot_of_a_point_inside_leans_towards_the_flatter_end() {
        let e = ellipse();
        let tol = tolerance();
        let inside = Point::from_vec3(local(2.0, 1.0, 0.0));
        let c = Curve::closest(&e, inside, None, tol, &mut Scratch::default()).unwrap();
        // Straight up, at π/2, is 4 away; the foot leans towards the flatter end and is nearer.
        assert!(c.at > 1.3 && c.at < FRAC_PI_2 && c.distance.millimetres() < 3.95, "{c:?}");
        let t = Curve::invert(&e, inside, None, tol, &mut Scratch::default()).unwrap();
        assert_eq!(t.to_bits(), c.at.to_bits());
        let arc = e.trimmed(PI, 1.5 * PI).unwrap();
        let near_start =
            Curve::closest(&arc, Point::from_vec3(local(-13.0, 1.0, 0.0)), None, tol, &mut Scratch::default());
        assert_eq!(near_start.unwrap().at, PI);
    }

    #[test]
    fn of_two_feet_as_near_the_first_from_the_start_is_taken() {
        // From the centre, the ends of the minor axis are both exactly the minor radius away.
        let flat =
            Ellipse::placed(&Placement::identity(), Length::from_millimetres(10.0), Length::from_millimetres(4.0));
        let centre = Point::from_vec3(occam_units::Vec3::new(0.0, 0.0, 0.0));
        let c = Curve::closest(&flat.unwrap(), centre, None, tolerance(), &mut Scratch::default()).unwrap();
        assert!((c.at - FRAC_PI_2).abs() < 1e-12 && c.distance.millimetres().to_bits() == 4f64.to_bits(), "{c:?}");
    }

    #[test]
    fn its_radii_must_be_positive() {
        let size =
            |x: f64, y: f64| Ellipse::placed(&placement(), Length::from_millimetres(x), Length::from_millimetres(y));
        assert_eq!(size(0.0, 5.0).unwrap_err().code, Code::SizeNotPositive);
        assert_eq!(size(12.0, -5.0).unwrap_err().code, Code::SizeNotPositive);
        assert!(size(5.0, 12.0).is_ok(), "either radius may be the larger");
    }

    #[test]
    fn its_shape_and_content_name_what_defines_it() {
        let e = ellipse().trimmed(0.5, 2.5).unwrap();
        let shape = Curve::shape(&e);
        assert_eq!((shape.form, shape.values.len()), ("ellipse", 7));
        assert_eq!((shape.values[3].number(), shape.values[4].number()), (Some(12.0), Some(5.0)));
        assert_eq!(shape.values[6], NamedValue::new("to", Quantity::Angle, 2.5));
        let mut content = Content::default();
        Curve::content(&e, &mut content);
        assert_eq!(content.words().len(), 24, "the name, the axes, two radii and the range");
        let b = Curve::bounds(&e, 0.0, TAU);
        assert!(
            b.contains(Point::from_vec3(local(12.0, 0.0, 0.0))) && b.contains(Point::from_vec3(local(0.0, -5.0, 0.0)))
        );
    }

    #[test]
    fn the_suite_catches_an_ellipse_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&conformance::WrongSecondCurve(ellipse()), 0.0, TAU));
        assert_eq!(failed, [Check::Differences]);
    }
}
