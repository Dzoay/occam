//! Source: original
//! The parabola: `vertex + t²/4f x + t y`, opening along `x` with focal length `f`, the parameter a length along `y`
//! (PARITY 2.1). Its foot is searched for, near where `y` alone would put it.

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Length, Local, ParamKind, Placement, Point, Tangent, World};

use crate::axes::Axes;
use crate::batch::Scratch;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::identity::{Content, FamilyName, Shape};
use crate::support::{enclose, exact, positive, span};

/// A parabola, unbounded or trimmed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parabola {
    axes: Axes,
    focal: f64,
    /// 1/4f and 1/2f, computed once.
    quarter: f64,
    half: f64,
    range: ParamRange,
}

impl Parabola {
    /// The parabola with its vertex at the origin of `placement`, opening along its X axis, in its XY plane.
    ///
    /// # Errors
    ///
    /// `geom.size-not-positive` for a focal length that is not a finite length greater than zero.
    pub fn placed(placement: &Placement<Local, World>, focal_length: Length) -> Result<Self, Report> {
        let focal = positive(focal_length)?;
        let range = ParamRange::unbounded(ParamKind::Length);
        Ok(Self { axes: Axes::of(placement), focal, quarter: 0.25 / focal, half: 0.5 / focal, range })
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

impl ParametricCurve for Parabola {
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        self.axes.at(t * t * R::splat(self.quarter), t)
    }

    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        (self.eval_point(t), Tangent::new(self.axes.along(t * R::splat(self.half), R::splat(1.0))))
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        for k in 1..=order {
            let (along_x, along_y) = match k {
                1 => (t * self.half, 1.0),
                2 => (self.half, 0.0),
                _ => (0.0, 0.0),
            };
            out.push(Tangent::new(self.axes.along(along_x, along_y)));
        }
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let square = |t: f64| exact(t) * exact(t);
        let (low, high) = (square(lo), square(hi));
        let least = if lo <= 0.0 && hi >= 0.0 {
            0.0
        } else if low.lower() < high.lower() {
            low.lower()
        } else {
            high.lower()
        };
        let most = if low.upper() > high.upper() { low.upper() } else { high.upper() };
        enclose([self.axes.reach(span(least, most) * exact(self.quarter), span(lo, hi))])
    }

    /// Where the foot of `p` can be: within the distance to the point at `p`'s own `y` of that `y`, since the curve's
    /// `y` is its parameter.
    fn search(&self, p: Point<World>) -> (f64, f64) {
        let along = self.axes.coordinates(p).1;
        let start = self.range.clamp(along);
        let reach = (self.eval_point(start).to_vec3() - p.to_vec3()).norm();
        (self.range.clamp(along - reach), self.range.clamp(along + reach))
    }

    fn family(&self) -> FamilyName {
        FamilyName("parabola")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push_curve(out, &[self.focal], self.range);
    }

    fn shape(&self) -> Shape {
        Shape {
            form: "parabola",
            values: self.axes.curve_values("vertex", &[NamedValue::length("focal-length", self.focal)], self.range),
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_report::Code;
    use occam_units::{Vec3, Vector};

    use super::*;
    use crate::conformance::{self, Check, agree, local, placement, tolerance};
    use crate::curve::Curve;

    fn parabola() -> Parabola {
        Parabola::placed(&placement(), Length::from_millimetres(3.0)).unwrap()
    }

    #[test]
    fn the_parabola_and_its_parts_pass_the_conformance_suite() {
        assert!(conformance::curve(&parabola(), -20.0, 30.0).is_empty());
        let part = parabola().trimmed(-5.0, 12.0).unwrap();
        let failures = conformance::curve(&part, -5.0, 12.0);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn named_parameters_give_closed_form_points_and_derivatives() {
        let p = parabola();
        assert!(agree(p.point(0.0).to_vec3(), local(0.0, 0.0, 0.0)), "the vertex");
        assert!(agree(p.point(6.0).to_vec3(), local(3.0, 6.0, 0.0)), "level with the focus, 2f from it");
        assert!(agree(p.point(-12.0).to_vec3(), local(12.0, -12.0, 0.0)));
        let along = |x: f64, y: f64| local(x, y, 0.0) - local(0.0, 0.0, 0.0);
        let mut out = vec![];
        Curve::derivatives(&p, 6.0, 4, &mut out, &mut Scratch::default()).unwrap();
        for (got, want) in out.iter().zip([along(1.0, 1.0), along(1.0 / 6.0, 0.0), along(0.0, 0.0), along(0.0, 0.0)]) {
            assert!(agree(got.rate(), want), "{got:?} {want:?}");
        }
        assert_eq!(p.first(6.0).1, out[0]);
    }

    #[test]
    fn the_foot_is_found_on_the_nearer_arm() {
        let p = parabola();
        let tol = tolerance();
        let on_axis = Point::from_vec3(local(20.0, 0.0, 0.0));
        let c = Curve::closest(&p, on_axis, None, tol, &mut Scratch::default()).unwrap();
        let want = (4.0 * 3.0 * (20.0 - 2.0 * 3.0)).sqrt();
        assert!((c.at.abs() - want).abs() < 1e-9, "a point on the axis past 2f is nearest two points: {c:?}");
        let part = p.trimmed(1.0, 4.0).unwrap();
        let t = Curve::invert(&part, Point::from_vec3(local(0.0, -3.0, 0.0)), None, tol, &mut Scratch::default());
        assert_eq!(t.unwrap(), 1.0);
        // Placed away from the world's origin, with the point at (-62500, -500) in its own frame, opposite through the
        // world's origin to the parabola's point at its `y`: the foot is by the vertex, far from that `y`.
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 500.0, 0.0)));
        let away = Parabola::placed(&Placement::identity().then(&shift), Length::from_millimetres(1.0)).unwrap();
        let behind = Point::from_vec3(Vec3::new(-62_500.0, 0.0, 0.0));
        let c = Curve::closest(&away, behind, None, tol, &mut Scratch::default()).unwrap();
        let to_vertex = (away.point(0.0).to_vec3() - behind.to_vec3()).norm();
        assert!(c.at.abs() < 0.1 && c.distance.millimetres() <= to_vertex, "{c:?}");
    }

    #[test]
    fn its_box_holds_the_part_and_stays_near_it() {
        let flat = Parabola::placed(&Placement::identity(), Length::from_millimetres(3.0)).unwrap();
        for ((lo, hi), (low_x, high_x)) in
            [((-6.0, 12.0), (0.0, 12.0)), ((2.0, 6.0), (1.0 / 3.0, 3.0)), ((-6.0, -2.0), (1.0 / 3.0, 3.0))]
        {
            let b = Curve::bounds(&flat, lo, hi);
            let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
            assert!(
                low.x <= low_x && low.x > low_x - 1e-9 && high.x >= high_x && high.x < high_x + 1e-9,
                "{lo} {hi}: {b:?}"
            );
            assert!(low.y <= lo && low.y > lo - 1e-9 && high.y >= hi && high.y < hi + 1e-9);
        }
    }

    #[test]
    fn its_focal_length_must_be_positive_and_its_shape_names_it() {
        assert_eq!(
            Parabola::placed(&placement(), Length::from_millimetres(0.0)).unwrap_err().code,
            Code::SizeNotPositive
        );
        let shape = Curve::shape(&parabola());
        assert_eq!((shape.form, shape.values.len()), ("parabola", 6));
        assert_eq!(shape.values[3], NamedValue::length("focal-length", 3.0));
        let mut content = Content::default();
        Curve::content(&parabola().trimmed(-5.0, 12.0).unwrap(), &mut content);
        assert_eq!(content.words().len(), 24, "the name, the axes, the focal length and the range");
        assert_eq!(content.words()[23], 12.0f64.to_bits());
    }

    #[test]
    fn the_suite_catches_a_parabola_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&conformance::WrongSecondCurve(parabola()), -20.0, 30.0));
        assert_eq!(failed, [Check::Differences]);
    }
}
