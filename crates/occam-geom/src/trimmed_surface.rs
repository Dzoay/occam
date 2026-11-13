//! Source: original
//! The trimmed surface: any surface restricted to a rectangle of its parameters (PARITY 2.2), its domain that rectangle
//! as [`ParamRange::trimmed`] makes it, and everything else the base's. Its foot is the base's where that lies inside,
//! as the nearest point of the whole is then the nearest of the part, and otherwise searched for over the rectangle.

use occam_math::{Level, Real};
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Point, Tangent, Tolerance, UvPoint, World};

use crate::analytic::Analytic;
use crate::batch::{Jet1, Jet2, Jets, Order, Scratch};
use crate::bspline::BsplineSurface;
use crate::domain::{Degeneracy, Domain, Param, ParamRange, UvBox, Wrap};
use crate::exact;
use crate::foot::foot;
use crate::held::{push_surface, with_surface};
use crate::identity::{Content, FamilyName, Shape};
use crate::surface::Parametric;

/// How many cells a side the foot's search over the rectangle samples.
const SAMPLES: u32 = 16;

/// A surface restricted to a rectangle of its parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct TrimmedSurface<S> {
    base: S,
    domain: Domain,
    /// The base's degenerate lines that cross the rectangle.
    lines: Vec<Degeneracy>,
}

/// `t` moved by whole periods of `base`, where it comes round, to at least `part`'s low end.
pub(crate) fn into(base: ParamRange, part: ParamRange, t: f64) -> f64 {
    if base.wrap != Wrap::Periodic {
        return t;
    }
    let period = base.hi - base.lo;
    let moved = t - ((t - part.lo) / period).floor() * period;
    if moved < part.lo { part.lo } else { moved }
}

impl<S: Parametric> TrimmedSurface<S> {
    /// `base` from `window.lo` to `window.hi` in each parameter.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`], for either parameter: an end outside the base's domain, not finite, a high end not
    /// above its low end, or more than a period of a parameter that comes round.
    pub fn new(base: S, window: UvBox) -> Result<Self, Report> {
        let whole = base.domain();
        let u = whole.u.trimmed(window.lo.u(), window.hi.u())?;
        let v = whole.v.trimmed(window.lo.v(), window.hi.v())?;
        let crosses =
            |line: &&Degeneracy| if line.param == Param::U { u.contains(line.at) } else { v.contains(line.at) };
        let lines = base.degeneracies().iter().filter(crosses).copied().collect();
        Ok(Self { base, domain: Domain { u, v }, lines })
    }

    /// The rectangle.
    fn window(&self) -> UvBox {
        let (u, v) = (self.domain.u, self.domain.v);
        UvBox::new(UvPoint::new(u.lo, v.lo), UvPoint::new(u.hi, v.hi))
    }
}

impl<S: Parametric> Parametric for TrimmedSurface<S> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.base.eval_point(uv)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        self.base.eval_first(uv)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        self.base.eval_second(uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        self.base.derivatives(uv, order, out, scratch)
    }

    /// The base's batch, at whatever width it runs, with a point outside the rectangle undefined.
    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, scratch: &mut Scratch, level: Level) {
        self.base.eval_batch(uv, order, out, scratch, level);
        for (defined, at) in out.defined.iter_mut().zip(uv) {
            *defined &= self.domain.contains(*at);
        }
    }

    fn domain(&self) -> Domain {
        self.domain
    }

    fn highest_derivative(&self) -> Option<u8> {
        self.base.highest_derivative()
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        self.base.bounds(patch)
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let whole = self.base.domain();
        let at = self.base.invert(p, seed, tol, scratch)?;
        let moved = UvPoint::new(into(whole.u, self.domain.u, at.u()), into(whole.v, self.domain.v, at.v()));
        if self.domain.contains(moved) {
            return Ok(moved);
        }
        let start = UvPoint::new(self.domain.u.clamp(moved.u()), self.domain.v.clamp(moved.v()));
        Ok(foot(self, p, self.window(), (SAMPLES, SAMPLES), Some(start), scratch))
    }

    fn family(&self) -> FamilyName {
        FamilyName("trimmed")
    }

    fn content(&self, out: &mut Content) {
        for x in [self.domain.u.lo, self.domain.u.hi, self.domain.v.lo, self.domain.v.hi] {
            out.push(x);
        }
        push_surface(out, &self.base);
    }

    fn shape(&self) -> Shape {
        let (u, v) = (self.domain.u, self.domain.v);
        let ends = vec![
            NamedValue::new("u-from", u.quantity(), u.lo),
            NamedValue::new("u-to", u.quantity(), u.hi),
            NamedValue::new("v-from", v.quantity(), v.lo),
            NamedValue::new("v-to", v.quantity(), v.hi),
        ];
        Shape { form: "trimmed", values: with_surface(ends, &self.base) }
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        &self.lines
    }

    /// The base's carrier, as a patch presents its carrier (S4).
    fn analytic(&self, tol: Tolerance) -> Option<Analytic> {
        self.base.analytic(tol)
    }

    /// The base's B-spline over a patch inside the rectangle.
    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        exact::part(self.domain, patch)?;
        self.base.to_bspline(patch)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use occam_report::Code;
    use occam_units::Length;

    use super::*;
    use crate::batch::Jets;
    use crate::conformance::{self, local, placement, tolerance};
    use crate::surface::Surface;
    use crate::{Cylinder, MeshSurface, Plane, Sphere, SplineSurface, workloads};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn window(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
        UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1))
    }

    fn cylinder() -> Cylinder {
        Cylinder::placed(&placement(), mm(6.0)).unwrap()
    }

    fn sphere() -> Sphere {
        Sphere::placed(&placement(), mm(5.0)).unwrap()
    }

    fn spline() -> SplineSurface {
        SplineSurface::new(&workloads::spline()).unwrap()
    }

    fn mesh() -> MeshSurface {
        let at = |u: f64, v: f64| UvPoint::new(u, v);
        let uv = vec![at(0.0, 0.0), at(1.0, 0.0), at(2.0, 0.0), at(0.0, 1.0), at(1.0, 1.0), at(2.0, 1.0)];
        let points: Vec<Point<World>> =
            uv.iter().map(|p| Point::from_vec3(local(3.0 * p.u(), 2.0 * p.v(), p.u() * p.v()))).collect();
        MeshSurface::new(&points, uv, &[[0, 1, 4], [0, 4, 3], [1, 2, 5], [1, 5, 4]]).unwrap()
    }

    fn cut<S: Parametric>(base: S, w: UvBox) -> TrimmedSurface<S> {
        TrimmedSurface::new(base, w).unwrap()
    }

    #[test]
    fn a_trimmed_surface_of_every_kind_passes_the_suite() {
        let cases: [(Box<dyn Surface>, UvBox); 6] = [
            (
                Box::new(cut(Plane::placed(&placement()), window((-3.0, 2.0), (4.0, 9.5)))),
                window((-3.0, 2.0), (4.0, 9.5)),
            ),
            (Box::new(cut(cylinder(), window((-1.0, -4.0), (2.5, 6.0)))), window((-1.0, -4.0), (2.5, 6.0))),
            (Box::new(cut(cylinder(), window((0.5, 0.0), (0.5 + TAU, 3.0)))), window((0.5, 0.0), (0.5 + TAU, 3.0))),
            (Box::new(cut(sphere(), window((0.3, 0.2), (2.0, FRAC_PI_2)))), window((0.3, 0.2), (2.0, FRAC_PI_2))),
            (Box::new(cut(spline(), window((0.2, 0.1), (0.7, 0.9)))), window((0.2, 0.1), (0.7, 0.9))),
            (Box::new(cut(mesh(), window((0.2, 0.0), (1.7, 0.8)))), window((0.2, 0.0), (1.7, 0.8))),
        ];
        for (s, w) in cases {
            let failures = conformance::surface(s.as_ref(), w);
            assert!(failures.is_empty(), "{:?} over {w:?}: {failures:#?}", s.shape().values);
        }
    }

    #[test]
    fn its_domain_is_the_rectangle_and_a_point_outside_is_undefined() {
        let s = cut(cylinder(), window((0.5, 0.0), (0.5 + TAU, 3.0)));
        assert_eq!(
            (Surface::domain(&s).u.wrap, Surface::domain(&s).v.wrap),
            (Wrap::Closed, Wrap::Open),
            "a whole turn"
        );
        let s = cut(spline(), window((0.2, 0.1), (0.7, 0.9)));
        let (mut out, mut scratch) = (vec![], Scratch::default());
        assert_eq!(
            Surface::derivatives(&s, UvPoint::new(0.8, 0.5), 1, &mut out, &mut scratch).unwrap_err().code,
            Code::UndefinedParameter
        );
        let mut jets = Jets::default();
        let uv = [UvPoint::new(0.3, 0.5), UvPoint::new(0.8, 0.5), UvPoint::new(0.5, 0.05), UvPoint::new(0.69, 0.89)];
        Surface::eval_batch(&s, &uv, Order::First, &mut jets, &mut scratch, Level::baseline());
        assert_eq!(jets.defined, [true, false, false, true]);
        assert_eq!(jets.points[1], s.point(uv[1]), "the base's point all the same");
    }

    #[test]
    fn its_foot_is_the_bases_inside_and_the_nearest_of_the_rectangle_outside() {
        let (s, tol) = (cut(cylinder(), window((0.0, 0.0), (1.0, 2.0))), tolerance());
        let inside = s.point(UvPoint::new(0.4, 1.5));
        let foot = Surface::invert(&s, inside, None, tol, &mut Scratch::default()).unwrap();
        assert!((foot.u() - 0.4).abs() < 1e-12 && (foot.v() - 1.5).abs() < 1e-12, "{foot:?}");
        // Opposite the patch, at the angle 3.5, nearer the edge at u = 1 than the one at u = 0, and above it.
        let (sin, cos) = occam_math::sin_cos(3.5);
        let beyond = Point::from_vec3(local(9.0 * cos, 9.0 * sin, 5.0));
        let found = Surface::closest(&s, beyond, None, tol, &mut Scratch::default()).unwrap();
        let mut nearest = f64::INFINITY;
        for i in 0..=200 {
            for j in 0..=200 {
                let at = UvPoint::new(f64::from(i) / 200.0, 2.0 * f64::from(j) / 200.0);
                let gap = (s.point(at).to_vec3() - beyond.to_vec3()).norm();
                nearest = if gap < nearest { gap } else { nearest };
            }
        }
        assert!(found.distance.millimetres() <= nearest + tol.band().millimetres(), "{found:?} against {nearest}");
        assert!(Surface::domain(&s).contains(found.at) && (found.at.u() - 1.0).abs() < 1e-9, "{found:?}");
    }

    #[test]
    fn it_keeps_the_lines_presents_and_bspline_of_its_base_inside_the_rectangle() {
        let s = cut(sphere(), window((0.3, 0.2), (2.0, FRAC_PI_2)));
        let lines: Vec<f64> = Surface::degeneracies(&s).iter().map(|line| line.at).collect();
        assert_eq!(lines, [FRAC_PI_2], "the north pole, not the south");
        assert_eq!(Surface::analytic(&s, tolerance()), Surface::analytic(&sphere(), tolerance()));
        let inside = window((0.5, 0.3), (1.5, 1.0));
        assert_eq!(Surface::to_bspline(&s, inside), Surface::to_bspline(&sphere(), inside));
        assert_eq!(Surface::to_bspline(&s, window((0.5, 0.0), (1.5, 1.0))), None, "past the rectangle");
        assert!(Surface::triangles(&cut(mesh(), window((0.2, 0.0), (1.7, 0.8)))).is_none());
    }

    #[test]
    fn a_rectangle_outside_the_domain_or_back_to_front_is_refused() {
        for w in [window((0.0, -2.0), (1.0, 0.0)), window((1.0, 0.0), (0.5, 1.0)), window((0.0, 0.0), (7.0, 1.0))] {
            assert_eq!(TrimmedSurface::new(sphere(), w).unwrap_err().code, Code::UndefinedParameter, "{w:?}");
        }
    }

    #[test]
    fn a_parameter_that_comes_round_is_moved_into_the_part() {
        let turn = ParamRange { lo: 0.0, hi: TAU, wrap: Wrap::Periodic, kind: occam_units::ParamKind::Angle };
        let part = ParamRange { lo: 5.0, hi: 7.0, wrap: Wrap::Open, kind: occam_units::ParamKind::Angle };
        assert!((into(turn, part, 0.5) - (0.5 + TAU)).abs() < 1e-15 && (into(turn, part, 5.5) - 5.5).abs() < 1e-15);
        let centred = ParamRange { lo: -PI, hi: PI, ..turn };
        assert!(
            (into(centred, part, 0.5) - (0.5 + TAU)).abs() < 1e-15,
            "a turn is the range's length, wherever it starts"
        );
        let open = ParamRange { wrap: Wrap::Open, ..turn };
        assert_eq!(into(open, part, 0.5), 0.5, "a parameter that does not come round stays");
        // Just below 17 turns, the quotient rounds to 17, and the turns taken away overshoot: held at the part's start.
        let from_zero = ParamRange { lo: 0.0, hi: 2.0, ..part };
        assert_eq!(into(turn, from_zero, (17.0 * TAU).next_down()), 0.0, "never below the part");
    }

    #[test]
    fn its_shape_and_content_name_the_rectangle_and_the_base() {
        let s = cut(cylinder(), window((-1.0, -4.0), (2.5, 6.0)));
        let shape = Surface::shape(&s);
        let names: Vec<&str> = shape.values.iter().take(5).map(|v| v.name.as_str()).collect();
        assert_eq!((shape.form, names), ("trimmed", vec!["u-from", "u-to", "v-from", "v-to", "base-origin"]));
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(&content.words()[8..12], [-1f64, 2.5, -4.0, 6.0].map(f64::to_bits));
        assert_eq!(
            (Surface::family(&s), Surface::highest_derivative(&cut(mesh(), window((0.2, 0.0), (1.7, 0.8))))),
            (FamilyName("trimmed"), Some(1))
        );
    }
}
