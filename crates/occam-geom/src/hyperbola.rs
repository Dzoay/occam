//! Source: original
//! The hyperbola: `centre + a cosh t x + b sinh t y`, the branch that opens along `x`, with the usual parameter, which
//! has no unit (PARITY 2.1; `docs/decisions/2026-11-11-exp-from-arm.md`). Its foot is searched for.

use occam_math::{Interval, Real, sinh_cosh};
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Local, ParamKind, Placement, Point, Tangent, World};

use crate::axes::Axes;
use crate::batch::Scratch;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::identity::{Content, FamilyName, Shape};
use crate::support::{enclose, exact, positive, span};

/// One branch of a hyperbola, unbounded or trimmed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hyperbola {
    axes: Axes,
    radii: (f64, f64),
    range: ParamRange,
}

/// An interval holding `lo` to `hi` and their neighbours sixteen machine epsilons out, for values the owned `sinh_cosh`
/// computed, which are within 1.6 ulps.
fn computed(lo: f64, hi: f64) -> Interval {
    span(lo - 16.0 * f64::EPSILON * lo.abs(), hi + 16.0 * f64::EPSILON * hi.abs())
}

impl Hyperbola {
    /// The branch about the origin of `placement`, in its XY plane, with its vertex `x_radius` along its X axis and its
    /// asymptotes' slope `y_radius` over `x_radius`.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a radius that is not a finite length greater than zero.
    pub fn placed(placement: &Placement<Local, World>, x_radius: Length, y_radius: Length) -> Result<Self, Report> {
        let range = ParamRange::unbounded(ParamKind::Knot);
        Ok(Self { axes: Axes::of(placement), radii: (positive(x_radius)?, positive(y_radius)?), range })
    }

    /// The part from parameter `lo` to `hi`.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`].
    pub fn trimmed(self, lo: f64, hi: f64) -> Result<Self, Report> {
        Ok(Self { range: self.range.trimmed(lo, hi)?, ..self })
    }
}

impl ParametricCurve for Hyperbola {
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        let (sinh, cosh) = sinh_cosh(t);
        self.axes.at(R::splat(self.radii.0) * cosh, R::splat(self.radii.1) * sinh)
    }

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        let (sinh, cosh) = sinh_cosh(t);
        let (a, b) = (R::splat(self.radii.0), R::splat(self.radii.1));
        (self.axes.at(a * cosh, b * sinh), Tangent::new(self.axes.along(a * sinh, b * cosh)))
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let (sinh, cosh) = sinh_cosh(t);
        for k in 1..=order {
            let (along_x, along_y) = if k % 2 == 1 { (sinh, cosh) } else { (cosh, sinh) };
            out.push(Tangent::new(self.axes.along(self.radii.0 * along_x, self.radii.1 * along_y)));
        }
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let (near, far) =
            if lo <= 0.0 && hi >= 0.0 { (0.0, larger(-lo, hi)) } else { smaller_larger(lo.abs(), hi.abs()) };
        let cosh = computed(sinh_cosh(near).1, sinh_cosh(far).1);
        let sinh = computed(sinh_cosh(lo).0, sinh_cosh(hi).0);
        enclose([self.axes.reach(exact(self.radii.0) * cosh, exact(self.radii.1) * sinh)])
    }

    /// Where the foot of `p` can be: no further than the point at the parameter nearest 0, so `a cosh t` is at most
    /// `|u|` and that distance, which bounds `|t|`.
    fn search(&self, p: Point<World>) -> (f64, f64) {
        let start = self.range.clamp(0.0);
        let reach = (self.eval_point(start).to_vec3() - p.to_vec3()).norm() + self.axes.coordinates(p).0.abs();
        // Doubled up to ten times, to 1024, where `a cosh t` is infinite.
        let mut limit = 1.0;
        for _ in 0..10 {
            if self.radii.0 * sinh_cosh(limit).1 >= reach {
                break;
            }
            limit *= 2.0;
        }
        (self.range.clamp(-limit), self.range.clamp(limit))
    }

    fn family(&self) -> FamilyName {
        FamilyName("hyperbola")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push_curve(out, &[self.radii.0, self.radii.1], self.range);
    }

    fn shape(&self) -> Shape {
        Shape {
            form: "hyperbola",
            values: self.axes.curve_values(
                "centre",
                &[NamedValue::length("x-radius", self.radii.0), NamedValue::length("y-radius", self.radii.1)],
                self.range,
            ),
        }
    }
}

/// The larger of two numbers, the first if they are equal.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The two numbers, smaller first.
fn smaller_larger(a: f64, b: f64) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

#[cfg(test)]
mod tests {
    use occam_report::{Code, Quantity};
    use occam_units::{Vec3, Vector};

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, tolerance};
    use crate::curve::Curve;

    fn hyperbola() -> Hyperbola {
        Hyperbola::placed(&placement(), Length::from_millimetres(4.0), Length::from_millimetres(3.0)).unwrap()
    }

    /// cosh 1 and sinh 1, from Python's `decimal`, rounded once.
    const COSH_1: f64 = 1.543_080_634_815_243_7;
    const SINH_1: f64 = 1.175_201_193_643_801_4;

    #[test]
    fn the_hyperbola_and_its_parts_pass_the_conformance_suite() {
        assert!(conformance::curve(&hyperbola(), -2.0, 2.5).is_empty());
        let part = hyperbola().trimmed(-1.0, 1.5).unwrap();
        let failures = conformance::curve(&part, -1.0, 1.5);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let h = hyperbola();
        assert!(agree(h.point(0.0).to_vec3(), local(4.0, 0.0, 0.0)), "the vertex");
        assert!(agree(h.point(1.0).to_vec3(), local(4.0 * COSH_1, 3.0 * SINH_1, 0.0)));
        assert!(agree(h.point(-1.0).to_vec3(), local(4.0 * COSH_1, -3.0 * SINH_1, 0.0)));
        let along = |x: f64, y: f64| local(x, y, 0.0) - local(0.0, 0.0, 0.0);
        let mut out = vec![];
        Curve::derivatives(&h, 1.0, 3, &mut out, &mut Scratch::default()).unwrap();
        let first = along(4.0 * SINH_1, 3.0 * COSH_1);
        for (got, want) in out.iter().zip([first, along(4.0 * COSH_1, 3.0 * SINH_1), first]) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
        assert_eq!(h.first(1.0).1, out[0]);
    }

    #[test]
    fn the_foot_is_found_far_out_on_either_arm() {
        let h = hyperbola();
        let tol = tolerance();
        for t in [-6.0, -0.3, 0.0, 2.0, 9.0] {
            let p = h.point(t);
            let off = Point::from_vec3(p.to_vec3() + (local(0.0, 0.0, 1.0) - local(0.0, 0.0, 0.0)) * 0.5);
            let c = Curve::closest(&h, off, None, tol, &mut Scratch::default()).unwrap();
            assert!(
                (c.at - t).abs() <= 1e-9 * (1.0 + t.abs()) && (c.distance.millimetres() - 0.5).abs() < 1e-9,
                "{t}: {c:?}"
            );
        }
        let part = h.trimmed(1.0, 2.0).unwrap();
        let below = Curve::invert(&part, h.point(-1.0), None, tol, &mut Scratch::default()).unwrap();
        assert_eq!(below, 1.0);
    }

    #[test]
    fn its_box_holds_an_arm_and_stays_near_it() {
        let flat =
            Hyperbola::placed(&Placement::identity(), Length::from_millimetres(4.0), Length::from_millimetres(3.0))
                .unwrap();
        let b = Curve::bounds(&flat, -1.0, 1.0);
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        assert!(low.x <= 4.0 && low.x > 4.0 - 1e-9 && high.x >= 4.0 * COSH_1 && high.x < 4.0 * COSH_1 + 1e-9, "{b:?}");
        assert!(low.y <= -3.0 * SINH_1 && low.y > -3.0 * SINH_1 - 1e-9 && high.y < 3.0 * SINH_1 + 1e-9);
        let one_side = Curve::bounds(&flat, 0.5, 1.0);
        assert!(one_side.low().x().millimetres() > 4.0, "cosh is above 1 away from 0");
        assert!(
            Curve::bounds(&flat, 0.0, f64::INFINITY)
                .contains(Point::from_vec3(occam_units::Vec3::new(1e300, 1e300, 0.0)))
        );
    }

    #[test]
    fn its_box_holds_every_part_closely_and_computed_values_sixteen_epsilons_out() {
        let (cosh_9, cosh_10) = (sinh_cosh(9.0).1, sinh_cosh(10.0).1);
        let held = computed(cosh_9, cosh_10);
        let below = (cosh_9 - held.lower()) / (f64::EPSILON * cosh_9);
        let above = (held.upper() - cosh_10) / (f64::EPSILON * cosh_10);
        assert!((16.0..=20.0).contains(&below) && (16.0..=20.0).contains(&above), "{below} {above}: {held:?}");
        let unit = Length::from_millimetres(1.0);
        let flat = Hyperbola::placed(&Placement::identity(), unit, unit).unwrap();
        // Across 0 lopsidedly, and on either side of it beyond 1, where cosh is least at the end nearer 0.
        for (lo, hi) in [(-3.0, 1.0), (1.5, 2.5), (-2.5, -1.5)] {
            let b = Curve::bounds(&flat, lo, hi);
            let points: Vec<Vec3> =
                (0..=1000).map(|k| flat.point(lo + (hi - lo) * f64::from(k) / 1000.0).to_vec3()).collect();
            let least =
                |pick: fn(&Vec3) -> f64| points.iter().map(pick).fold(f64::INFINITY, |a, x| if x < a { x } else { a });
            let most = |pick: fn(&Vec3) -> f64| {
                points.iter().map(pick).fold(f64::NEG_INFINITY, |a, x| if x > a { x } else { a })
            };
            let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
            for (face, sampled) in [(low.x, least(|v| v.x)), (low.y, least(|v| v.y))] {
                assert!(face <= sampled && sampled - face < 1e-6, "{lo} {hi}: {b:?}");
            }
            for (face, sampled) in [(high.x, most(|v| v.x)), (high.y, most(|v| v.y))] {
                assert!(face >= sampled && face - sampled < 1e-6, "{lo} {hi}: {b:?}");
            }
        }
    }

    #[test]
    fn the_search_reaches_the_foot_wherever_the_hyperbola_is_and_however_small() {
        let (tol, unit) = (tolerance(), Length::from_millimetres(1.0));
        let closest =
            |h: &Hyperbola, p: Vec3| Curve::closest(h, Point::from_vec3(p), None, tol, &mut Scratch::default());
        // Placed away from the world's origin, with the point opposite the vertex through it, at (-1, -2000) in the
        // hyperbola's own frame: the foot is far down the lower arm (from Python, by golden section).
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 1000.0, 0.0)));
        let away = Hyperbola::placed(&Placement::identity().then(&shift), unit, unit).unwrap();
        let c = closest(&away, Vec3::new(-1.0, -1000.0, 0.0)).unwrap();
        assert!(
            (c.at + 7.600_402_571_8).abs() < 1e-6 && (c.distance.millimetres() - 1_414.921_022_9).abs() < 1e-6,
            "{c:?}"
        );
        // Barely across the axis line and far up it.
        let flat = Hyperbola::placed(&Placement::identity(), unit, unit).unwrap();
        let c = closest(&flat, Vec3::new(1e-3, 500.0, 0.0)).unwrap();
        assert!(
            (c.at - 6.214_614_113_7).abs() < 1e-6 && (c.distance.millimetres() - 353.554_097_694_4).abs() < 1e-6,
            "{c:?}"
        );
        // Under a millimetre across, so that `a cosh t` passes the point only far along.
        let small = Length::from_millimetres(0.01);
        let tiny = Hyperbola::placed(&Placement::identity(), small, small).unwrap();
        let c = closest(&tiny, tiny.point(9.0).to_vec3()).unwrap();
        assert!((c.at - 9.0).abs() < 1e-9 && c.distance.millimetres() < 1e-9, "{c:?}");
        // Far enough out that the limit must be doubled to 32: ten steps of 2 would stop at 21.
        let far = flat.point(25.0).to_vec3();
        let c = closest(&flat, far).unwrap();
        assert!((c.at - 25.0).abs() < 1e-9 && c.distance.millimetres() < 1e-12 * far.norm(), "{c:?}");
    }

    #[test]
    fn its_radii_must_be_positive_and_its_shape_names_them() {
        let refused = Hyperbola::placed(&placement(), Length::from_millimetres(-4.0), Length::from_millimetres(3.0));
        assert_eq!(refused.unwrap_err().code, Code::SizeNotPositive);
        let part = hyperbola().trimmed(-1.0, 1.5).unwrap();
        let shape = Curve::shape(&part);
        assert_eq!((shape.form, shape.values.len()), ("hyperbola", 7));
        assert_eq!(shape.values[6], NamedValue::new("to", Quantity::Ratio, 1.5));
        let mut content = Content::default();
        Curve::content(&part, &mut content);
        assert_eq!(content.words().len(), 26, "the name, the axes, two radii and the range");
    }

    #[test]
    fn the_suite_catches_a_hyperbola_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&conformance::WrongSecondCurve(hyperbola()), -2.0, 2.5));
        assert_eq!(failed, [Check::Differences]);
    }
}
