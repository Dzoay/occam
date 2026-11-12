//! Source: piegl-tiller-1997
//! The exact B-spline forms of the analytic families, over a part of their domain
//! (`docs/decisions/2026-11-11-to-bspline-over-a-range.md`): a conic as rational quadratic arcs, each with its shoulder
//! pole where the tangents at its ends meet (Piegl and Tiller, section 7.5, A7.1 for the circle), and a surface turned
//! about an axis as its meridian's poles turned through the circle's (A8.1). The knots are the family's parameters at the
//! arcs' ends, so the forms agree with the families there.

use std::f64::consts::FRAC_PI_2;

use occam_math::{sin_cos, sinh_cosh};
use occam_units::{Point, Vec3};

use crate::axes::Axes;
use crate::bspline::{BsplineCurve, BsplineSurface};
use crate::domain::{Domain, ParamRange, UvBox};
use crate::meridian::Meridian;

/// A B-spline's knots, poles and weights, poles as vectors.
pub(crate) type Parts = (Vec<f64>, Vec<Vec3>, Vec<f64>);

/// Which conic an arc lies on, by the pair `(s, c)` its parameter gives: `sin_cos` for an ellipse and `sinh_cosh` for a
/// hyperbola, so that its point is `centre + c a + s b`.
#[derive(Clone, Copy)]
pub(crate) enum Conic {
    /// `cos` and `sin`, in arcs of at most a quarter turn.
    Elliptic,
    /// `cosh` and `sinh`, in arcs of at most a unit of the parameter.
    Hyperbolic,
}

impl Conic {
    /// `(s, c)` at `t`.
    fn pair(self, t: f64) -> (f64, f64) {
        match self {
            Self::Elliptic => sin_cos(t),
            Self::Hyperbolic => sinh_cosh(t),
        }
    }

    /// The longest arc.
    const fn piece(self) -> f64 {
        match self {
            Self::Elliptic => FRAC_PI_2,
            Self::Hyperbolic => 1.0,
        }
    }
}

/// `centre + c(t) a + s(t) b` for `t` from `lo` to `hi` as rational quadratic arcs. Each arc's shoulder pole is its middle
/// point over `c` of half the arc, with that as its weight: `cos` of half the angle on an ellipse, and `cosh` on a
/// hyperbola. `None` where a number is not finite.
pub(crate) fn conic(conic: Conic, (centre, a, b): (Vec3, Vec3, Vec3), (lo, hi): (f64, f64)) -> Option<Parts> {
    let at = |t: f64| {
        let (s, c) = conic.pair(t);
        centre + a * c + b * s
    };
    let finite = |v: &Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
    let (ends, ordered) = ([at(lo), at(hi)], lo < hi);
    if !ordered || !ends.iter().all(finite) {
        return None;
    }
    let mut arcs = 1_u32;
    while f64::from(arcs) * conic.piece() < hi - lo {
        arcs = arcs.saturating_add(1);
    }
    let step = (hi - lo) / f64::from(arcs);
    let weight = conic.pair(0.5 * step).1;
    let (mut knots, mut poles, mut weights) = (vec![lo; 3], vec![at(lo)], vec![1.0]);
    for k in 1..=arcs {
        let end = if k == arcs { hi } else { lo + step * f64::from(k) };
        let (s, c) = conic.pair(end - 0.5 * step);
        poles.extend([centre + (a * c + b * s) * (1.0 / weight), at(end)]);
        weights.extend([weight, 1.0]);
        knots.extend(std::iter::repeat_n(end, if k == arcs { 3 } else { 2 }));
    }
    poles.iter().all(finite).then_some((knots, poles, weights))
}

/// The conic with `radii` along the first two of `axes`, with `range`, from `lo` to `hi`, if it could be trimmed there.
pub(crate) fn conic_curve(
    which: Conic,
    (axes, radii): (&Axes, (f64, f64)),
    range: ParamRange,
    (lo, hi): (f64, f64),
) -> Option<BsplineCurve> {
    let part = range.trimmed(lo, hi).ok()?;
    let along = (axes.origin, axes.x * radii.0, axes.y * radii.1);
    Some(curve(2, conic(which, along, (part.lo, part.hi))?, true))
}

/// A curve from its parts.
pub(crate) fn curve(degree: u16, (knots, poles, weights): Parts, rational: bool) -> BsplineCurve {
    let poles = poles.into_iter().map(Point::from_vec3).collect();
    BsplineCurve { degree, knots, poles, weights: rational.then_some(weights) }
}

/// The polynomial B-spline of degree `degree` and one span from `lo` to `hi` with these poles.
pub(crate) fn span(degree: u16, (lo, hi): (f64, f64), poles: Vec<Vec3>) -> Parts {
    let ends = usize::from(degree).saturating_add(1);
    let knots = std::iter::repeat_n(lo, ends).chain(std::iter::repeat_n(hi, ends)).collect();
    let weights = vec![1.0; poles.len()];
    (knots, poles, weights)
}

/// The two ranges of `patch`, if a family with `domain` could be trimmed to it: finite, inside, with each high end
/// above its low end, and within a period of a parameter that comes round.
pub(crate) fn part(domain: Domain, patch: UvBox) -> Option<(ParamRange, ParamRange)> {
    Some((domain.u.trimmed(patch.lo.u(), patch.hi.u()).ok()?, domain.v.trimmed(patch.lo.v(), patch.hi.v()).ok()?))
}

/// The surface with `domain` turned from `meridian` about `axes`, over `patch`, if it could be trimmed to it: the
/// meridian's B-spline in the axes' coordinates, turned by [`revolved`].
pub(crate) fn turned(axes: &Axes, meridian: Meridian, domain: Domain, patch: UvBox) -> Option<BsplineSurface> {
    let (u, v) = part(domain, patch)?;
    let v = (v.lo, v.hi);
    let profile = match meridian {
        Meridian::Straight { start, slope } => {
            let at = |t: f64| Vec3::new(start + t * slope.0, 0.0, t * slope.1);
            (1, span(1, v, vec![at(v.0), at(v.1)]))
        }
        Meridian::Round { centre, radius } => {
            let (x, z) = (Vec3::new(radius, 0.0, 0.0), Vec3::new(0.0, 0.0, radius));
            (2, conic(Conic::Elliptic, (Vec3::new(centre, 0.0, 0.0), x, z), v)?)
        }
    };
    revolved(axes, (u.lo, u.hi), profile)
}

/// A curve's B-spline as a profile to turn about `axes`: its degree and parts, each pole `(a, b, h)` its coordinates
/// along `axes`' first, second and third axes from the origin.
pub(crate) fn profile(axes: &Axes, form: BsplineCurve) -> (u16, Parts) {
    let weights = form.weights.unwrap_or_else(|| vec![1.0; form.poles.len()]);
    let poles = form.poles.into_iter().map(|p| axes.local(p)).collect();
    (form.degree, (form.knots, poles, weights))
}

/// The surface turning `profile`, its degree and parts with each pole `(a, b, h)` in `axes`' coordinates, about
/// `axes`' normal `z` from angle `u.0` to `u.1`. Each pole of the profile and `e` of the unit circle's arcs give the
/// pole `origin + h z + a e + b (z × e)`, the profile's pole turned as `e` is, weighted by the product of their
/// weights (A8.1): turning is linear in a pole, so the rational circle carries every point of the profile round.
pub(crate) fn revolved(
    axes: &Axes,
    u: (f64, f64),
    (degree_v, (knots_v, line, along)): (u16, Parts),
) -> Option<BsplineSurface> {
    let (knots_u, circle, round) = conic(Conic::Elliptic, (Vec3::new(0.0, 0.0, 0.0), axes.x, axes.y), u)?;
    let (origin, up, circle) = (axes.origin, axes.normal.get(), &circle);
    let turn =
        |m: Vec3| circle.iter().map(move |e| Point::from_vec3(origin + up * m.z + *e * m.x + up.cross(*e) * m.y));
    let poles = line.iter().copied().flat_map(turn).collect();
    let weights = along.iter().flat_map(|wv| round.iter().map(move |wu| wu * wv)).collect();
    let size = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Some(BsplineSurface {
        degree: (2, degree_v),
        count: (size(knots_u.len().saturating_sub(3)), size(line.len())),
        knots_u,
        knots_v,
        poles,
        weights: Some(weights),
    })
}

/// The surface `form` swept along `direction` from `v.0` to `v.1`: the curve's poles moved to each end, the curve's
/// knots in `u` and one linear span in `v`, each row with the curve's weights.
pub(crate) fn extruded(form: BsplineCurve, direction: Vec3, v: (f64, f64)) -> BsplineSurface {
    let moved = |by: f64| form.poles.iter().map(move |p| Point::from_vec3(p.to_vec3() + direction * by));
    let poles = moved(v.0).chain(moved(v.1)).collect();
    let weights = form.weights.as_ref().map(|w| w.iter().chain(w).copied().collect());
    let count = (u32::try_from(form.poles.len()).unwrap_or(u32::MAX), 2);
    let knots_v = vec![v.0, v.0, v.1, v.1];
    BsplineSurface { degree: (form.degree, 1), count, knots_u: form.knots, knots_v, poles, weights }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{PI, TAU};

    use occam_units::{Angle, Direction, Length, Placement, UvPoint};

    use super::*;
    use crate::conformance::{self, placement};
    use crate::{Circle, Cone, Curve, Cylinder, Ellipse, Hyperbola, Line, Parabola, Plane, Sphere, Surface, Torus};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn patch(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
        UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1))
    }

    /// Every analytic surface, with patches across a seam, over a whole turn, through an apex and from pole to pole.
    fn surfaces() -> Vec<(Box<dyn Surface>, UvBox)> {
        let at = placement();
        let cone = || Cone::placed(&at, mm(3.0), Angle::from_radians(0.4)).unwrap();
        vec![
            (Box::new(Plane::placed(&at)), patch((-3.0, 2.0), (4.0, 9.5))),
            (Box::new(Cylinder::placed(&at, mm(6.0)).unwrap()), patch((-1.0, -15.0), (2.5, 25.0))),
            (Box::new(Cylinder::placed(&at, mm(6.0)).unwrap()), patch((0.3, 0.0), (0.3 + TAU, 1.0))),
            (Box::new(cone()), patch((0.5, -12.0), (4.0, 6.0))),
            (Box::new(Sphere::placed(&at, mm(5.0)).unwrap()), patch((0.0, -PI / 2.0), (TAU, PI / 2.0))),
            (Box::new(Sphere::placed(&at, mm(5.0)).unwrap()), patch((-2.0, -0.4), (1.0, 1.2))),
            (Box::new(Torus::placed(&at, mm(8.0), mm(2.0)).unwrap()), patch((1.0, -1.0), (5.5, 4.0))),
        ]
    }

    /// Every analytic curve, with arcs across a seam, a whole turn, and parts of the unbounded ones.
    fn curves() -> Vec<(Box<dyn Curve>, (f64, f64))> {
        let at = placement();
        let line =
            Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(1.0, -2.0, 0.5)));
        vec![
            (Box::new(line), (-4.0, 7.5)),
            (Box::new(Circle::placed(&at, mm(10.0)).unwrap()), (0.0, TAU)),
            (Box::new(Circle::placed(&at, mm(10.0)).unwrap().trimmed(5.0, 7.0).unwrap()), (5.5, 7.0)),
            (Box::new(Ellipse::placed(&at, mm(7.0), mm(3.0)).unwrap()), (-1.0, 4.0)),
            (Box::new(Hyperbola::placed(&at, mm(2.0), mm(5.0)).unwrap()), (-2.5, 3.2)),
            (Box::new(Parabola::placed(&at, mm(1.5)).unwrap()), (-6.0, 4.0)),
        ]
    }

    #[test]
    fn every_analytic_family_converted_to_a_bspline_agrees_with_itself() {
        for (s, window) in surfaces() {
            assert!(s.to_bspline(window).is_some(), "{:?}", s.family());
            let failures = conformance::presented_surface(s.as_ref(), window);
            assert!(failures.is_empty(), "{:?} over {window:?}: {failures:#?}", s.family());
        }
        for (c, (lo, hi)) in curves() {
            assert!(c.to_bspline(lo, hi).is_some(), "{:?}", c.family());
            let failures = conformance::presented_curve(c.as_ref(), lo, hi);
            assert!(failures.is_empty(), "{:?} from {lo} to {hi}: {failures:#?}", c.family());
        }
    }

    #[test]
    fn no_bspline_is_given_where_the_part_is_unbounded_outside_or_more_than_a_turn() {
        let at = placement();
        let plane = Plane::placed(&Placement::identity());
        assert_eq!(Surface::to_bspline(&plane, patch((0.0, f64::NEG_INFINITY), (1.0, 1.0))), None);
        let cylinder = Cylinder::placed(&at, mm(6.0)).unwrap();
        assert_eq!(Surface::to_bspline(&cylinder, patch((0.0, 0.0), (TAU + 0.1, 1.0))), None, "more than a turn");
        assert_eq!(Surface::to_bspline(&cylinder, patch((0.0, 0.0), (1.0, f64::INFINITY))), None);
        let sphere = Sphere::placed(&at, mm(5.0)).unwrap();
        assert_eq!(Surface::to_bspline(&sphere, patch((0.0, -2.0), (1.0, 0.0))), None, "past a pole");
        assert_eq!(Surface::to_bspline(&sphere, patch((1.0, 0.0), (0.5, 1.0))), None, "back to front");
        let line =
            Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        assert_eq!(Curve::to_bspline(&line, 0.0, f64::INFINITY), None);
        let arc = Circle::placed(&at, mm(1.0)).unwrap().trimmed(0.5, 2.0).unwrap();
        assert_eq!(Curve::to_bspline(&arc, 0.0, 1.0), None, "outside the arc");
        let hyperbola = Hyperbola::placed(&at, mm(1.0), mm(1.0)).unwrap();
        assert_eq!(Curve::to_bspline(&hyperbola, 0.0, 800.0), None, "past the largest double");
    }

    #[test]
    fn arcs_are_at_most_a_quarter_turn_with_knots_at_their_ends() {
        let (x, y) = (Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 2.0, 0.0));
        let (knots, poles, weights) = conic(Conic::Elliptic, (Vec3::new(0.0, 0.0, 0.0), x, y), (0.0, TAU)).unwrap();
        assert_eq!(knots.len(), 12);
        assert_eq!((knots[3], knots[5], knots[7], knots[11]), (FRAC_PI_2, PI, 1.5 * PI, TAU));
        assert_eq!((poles.len(), weights.len()), (9, 9));
        assert!((weights[1] - 0.5f64.sqrt()).abs() < 1e-15);
        assert_eq!(weights[2], 1.0);
        assert!((poles[1] - Vec3::new(2.0, 2.0, 0.0)).norm() < 1e-14, "{:?}", poles[1]);
        let (knots, poles, _) = conic(Conic::Elliptic, (Vec3::new(0.0, 0.0, 0.0), x, y), (0.25, 1.5)).unwrap();
        assert_eq!((knots, poles.len()), (vec![0.25, 0.25, 0.25, 1.5, 1.5, 1.5], 3), "one arc");
    }

    #[test]
    fn a_hyperbolic_arc_puts_its_shoulder_where_the_tangents_meet() {
        let (x, y) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let (knots, poles, weights) = conic(Conic::Hyperbolic, (Vec3::new(0.0, 0.0, 0.0), x, y), (-0.5, 0.5)).unwrap();
        assert_eq!(knots, [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]);
        let (sinh, cosh) = sinh_cosh(0.5);
        assert!((poles[1] - Vec3::new(1.0 / cosh, 0.0, 0.0)).norm() < 1e-15 && (weights[1] - cosh).abs() < 1e-15);
        assert!((poles[0] - Vec3::new(cosh, -sinh, 0.0)).norm() < 1e-15);
        assert_eq!(
            conic(Conic::Hyperbolic, (Vec3::new(0.0, 0.0, 0.0), x, y), (0.0, 800.0)),
            None,
            "past the largest double"
        );
        assert_eq!(conic(Conic::Elliptic, (Vec3::new(0.0, 0.0, 0.0), x, y), (1.0, 1.0)), None, "empty");
        // Ends of 1.5e308 along x, but a shoulder pole of 1.7e308 over cos 0.5, past the largest double, there alone.
        let huge = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.7e308, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        assert!(conic(Conic::Elliptic, huge, (-0.5, 0.5)).is_none(), "a shoulder pole past the largest double");
        assert!(conic(Conic::Elliptic, huge, (-0.5, -0.4)).is_some(), "short enough to keep its shoulder finite");
    }
}
