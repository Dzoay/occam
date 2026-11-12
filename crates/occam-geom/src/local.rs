//! Source: do-carmo-1976
//! What a curve and a surface are like at one parameter (PARITY 2.8). A curve's unit tangent, curvature, principal
//! normal and torsion are its Frenet trihedron's (do Carmo, chapter 1); a surface's unit normal, principal curvatures and
//! directions, and mean and Gaussian curvature come from its first and second fundamental forms (chapter 3). Each is read
//! through `dyn Curve` or `dyn Surface`, so every family is measured alike. Where a direction does not exist it is
//! reported, or is `None` where its absence is an ordinary answer: never NaN.
//!
//! A curvature is positive where the curve or surface bends towards its normal: a surface's normal is `Su x Sv`
//! normalised, so a sphere whose normal points out has curvature `-1/r`.

use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Direction, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::batch::Scratch;
use crate::curve::Curve;
use crate::surface::Surface;

/// A curve at one parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveLocal {
    /// The point.
    pub point: Point<World>,
    /// The unit tangent, the way the parameter runs.
    pub tangent: Direction<World>,
    /// How sharply it turns, per millimetre: zero where it is straight.
    pub curvature: f64,
    /// The principal normal, towards which it turns: `None` where it is straight within the angular tolerance.
    pub normal: Option<Direction<World>>,
    /// How sharply it leaves its osculating plane, per millimetre: `None` where it has no principal normal.
    pub torsion: Option<f64>,
}

/// A surface at one parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceLocal {
    /// The point.
    pub point: Point<World>,
    /// The unit normal, `Su x Sv` normalised.
    pub normal: Direction<World>,
    /// The principal curvatures, per millimetre, the larger first.
    pub curvatures: (f64, f64),
    /// The principal directions, in the order of the curvatures: `None` at an umbilic, where every direction is one.
    pub directions: Option<[Direction<World>; 2]>,
    /// The mean curvature, half their sum.
    pub mean: f64,
    /// The Gaussian curvature, their product.
    pub gaussian: f64,
}

/// The refusal of a direction where the derivatives span `span`, a ratio that is zero or near it; zero where it is not
/// a number at all.
pub(crate) fn singular(span: f64) -> Report {
    let span = if span.is_finite() { span } else { 0.0 };
    Report::new(Code::SingularPoint, Fault::InvalidInput).with_value(NamedValue::new("span", Quantity::Ratio, span))
}

/// Each tangent's rate, NaN past the last.
fn rates<const N: usize>(out: &[Tangent<World>]) -> [Vec3; N] {
    let nan = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
    std::array::from_fn(|k| out.get(k).map_or(nan, |t| t.rate()))
}

/// Whether `a` and `b` are parallel within the angle `theta`, or either is zero: `|a x b| <= theta |a| |b|`.
pub(crate) fn parallel(a: Vec3, b: Vec3, theta: f64) -> bool {
    let across = a.cross(b).norm();
    let ok = across > theta * a.norm() * b.norm();
    !ok
}

/// The curve at `t`, which lies in its range.
///
/// # Errors
///
/// `geom.undefined-parameter` outside the range, and `geom.singular-point` where the first derivative is zero or not
/// finite, so that there is no tangent.
pub fn curve_local(c: &dyn Curve, t: f64, tol: Tolerance, scratch: &mut Scratch) -> Result<CurveLocal, Report> {
    let mut out = Vec::with_capacity(3);
    c.derivatives(t, 3, &mut out, scratch)?;
    let [first, second, third] = rates::<3>(&out);
    let speed = first.norm();
    let moving = speed > 0.0 && speed.is_finite();
    if !moving {
        return Err(singular(speed));
    }
    let across = first.cross(second);
    let curvature = across.norm() / (speed * speed * speed);
    let straight = parallel(first, second, tol.angle().radians());
    let normal = (!straight).then(|| Direction::normalised(across.cross(first)));
    let torsion = (!straight).then(|| across.dot(third) / across.dot(across));
    Ok(CurveLocal { point: c.point(t), tangent: Direction::normalised(first), curvature, normal, torsion })
}

/// The first and second fundamental forms' coefficients at a point, and the unit normal.
struct Forms {
    first: (f64, f64, f64),
    second: (f64, f64, f64),
    normal: Vec3,
    rates: (Vec3, Vec3),
}

/// Whether the first derivatives `su` and `sv` span no tangent plane: parallel within the angle `theta`, not finite, or
/// one of them rounding away from zero against the other, within 64 machine epsilons of it, as at a pole or an apex,
/// where a direction computed from it would be noise.
pub(crate) fn no_plane(su: Vec3, sv: Vec3, theta: f64) -> bool {
    let (a, b) = (su.norm(), sv.norm());
    let (short, long) = if b < a { (b, a) } else { (a, b) };
    let vanishing = short <= 64.0 * f64::EPSILON * long;
    parallel(su, sv, theta) || vanishing || !su.cross(sv).norm().is_finite()
}

/// The forms of the derivatives `[su, sv, suu, suv, svv]`, or `None` where they span [`no_plane`].
fn forms([su, sv, suu, suv, svv]: [Vec3; 5], theta: f64) -> Option<Forms> {
    let across = su.cross(sv);
    if no_plane(su, sv, theta) {
        return None;
    }
    let normal = across * (1.0 / across.norm());
    let first = (su.dot(su), su.dot(sv), sv.dot(sv));
    let second = (suu.dot(normal), suv.dot(normal), svv.dot(normal));
    Some(Forms { first, second, normal, rates: (su, sv) })
}

/// The principal direction of curvature `k`: the solution of `(II - k I) w = 0` from whichever row of it is the
/// better conditioned, carried from `(du, dv)` into space.
#[expect(clippy::many_single_char_names, reason = "the fundamental forms' coefficients, as do Carmo names them")]
fn direction(f: &Forms, k: f64) -> Direction<World> {
    let ((e, ff, g), (l, m, n), (su, sv)) = (f.first, f.second, f.rates);
    let along = |du: f64, dv: f64| su * du + sv * dv;
    let (a, b) = (along(m - k * ff, k * e - l), along(n - k * g, k * ff - m));
    Direction::normalised(if b.norm() > a.norm() { b } else { a })
}

/// The surface at `uv`, which lies in its domain.
///
/// # Errors
///
/// `geom.undefined-parameter` outside the domain, `geom.no-derivative` for a surface without second derivatives, as a
/// mesh, and `geom.singular-point` where `Su` and `Sv` are parallel within the angular tolerance or either is zero, as at
/// a pole or an apex, where the forms that every measure here needs degenerate.
#[expect(clippy::many_single_char_names, reason = "the fundamental forms' coefficients, as do Carmo names them")]
pub fn surface_local(
    s: &dyn Surface,
    uv: UvPoint,
    tol: Tolerance,
    scratch: &mut Scratch,
) -> Result<SurfaceLocal, Report> {
    let mut out = Vec::with_capacity(5);
    s.derivatives(uv, 2, &mut out, scratch)?;
    let jets = rates::<5>(&out);
    let theta = tol.angle().radians();
    let [su, sv, ..] = jets;
    let Some(f) = forms(jets, theta) else { return Err(singular(su.cross(sv).norm() / (su.norm() * sv.norm()))) };
    let ((e, ff, g), (l, m, n)) = (f.first, f.second);
    let det = e * g - ff * ff;
    let gaussian = (l * n - m * m) / det;
    let mean = (e * n - 2.0 * ff * m + g * l) / (2.0 * det);
    let spread = mean * mean - gaussian;
    let half = if spread > 0.0 { spread.sqrt() } else { 0.0 };
    let curvatures = (mean + half, mean - half);
    let largest = if curvatures.0.abs() > curvatures.1.abs() { curvatures.0.abs() } else { curvatures.1.abs() };
    let distinct = half > theta * largest;
    let directions = distinct.then(|| [direction(&f, curvatures.0), direction(&f, curvatures.1)]);
    let normal = Direction::normalised(f.normal);
    Ok(SurfaceLocal { point: s.point(uv), normal, curvatures, directions, mean, gaussian })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use occam_units::{Angle, Length};

    use super::*;
    use crate::conformance::{forms, local, placement, tolerance};
    use crate::{
        Circle, Cone, Cylinder, Ellipse, Extrusion, Hyperbola, Line, Parabola, Plane, Sphere, SplineCurve,
        SplineSurface, Torus,
    };

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn close(got: f64, want: f64) -> bool {
        (got - want).abs() <= 1e-12 * (1.0 + want.abs())
    }

    fn on_curve(c: &dyn Curve, t: f64) -> CurveLocal {
        curve_local(c, t, tolerance(), &mut Scratch::default()).unwrap()
    }

    fn on_surface(s: &dyn Surface, uv: UvPoint) -> SurfaceLocal {
        surface_local(s, uv, tolerance(), &mut Scratch::default()).unwrap()
    }

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    #[test]
    fn known_curvatures_for_every_analytic_curve_family() {
        let at = placement();
        let along = Direction::normalised(Vec3::new(1.0, -2.0, 0.5));
        let straight = on_curve(&Line::new(point(1.0, 2.0, 3.0), along), 2.5);
        assert_eq!((straight.curvature, straight.normal, straight.torsion), (0.0, None, None));
        assert!((straight.tangent.get() - along.get()).norm() < 1e-15);
        let round = on_curve(&Circle::placed(&at, mm(4.0)).unwrap(), 1.1);
        assert!(close(round.curvature, 0.25) && round.torsion.is_some_and(|t| t.abs() < 1e-12), "{round:?}");
        let inward = local(0.0, 0.0, 0.0) - round.point.to_vec3();
        assert!((round.normal.unwrap().get() - inward * 0.25).norm() < 1e-12, "towards the centre");
        // At the ends of an ellipse's axes a / b^2 and b / a^2; at a hyperbola's vertex a / b^2; at a parabola's 1 / 2f.
        let ellipse = Ellipse::placed(&at, mm(7.0), mm(3.0)).unwrap();
        assert!(close(on_curve(&ellipse, 0.0).curvature, 7.0 / 9.0));
        assert!(close(on_curve(&ellipse, FRAC_PI_2).curvature, 3.0 / 49.0));
        assert!(close(on_curve(&Hyperbola::placed(&at, mm(2.0), mm(5.0)).unwrap(), 0.0).curvature, 2.0 / 25.0));
        assert!(close(on_curve(&Parabola::placed(&at, mm(1.5)).unwrap(), 0.0).curvature, 1.0 / 3.0));
    }

    #[test]
    fn a_twisted_cubic_has_the_curvature_and_torsion_of_its_formula() {
        // (t, t^2, t^3): at 0 its curvature is 2 and its torsion 3.
        let third = 1.0 / 3.0;
        let poles =
            vec![point(0.0, 0.0, 0.0), point(third, 0.0, 0.0), point(2.0 * third, third, 0.0), point(1.0, 1.0, 1.0)];
        let start = on_curve(&SplineCurve::bezier(poles, None).unwrap(), 0.0);
        assert!(close(start.curvature, 2.0) && close(start.torsion.unwrap(), 3.0), "{start:?}");
    }

    /// The expected principal curvatures, the larger first, from those where the normal points out along `outward`.
    fn signed(out: (f64, f64), outward: Vec3, found: &SurfaceLocal) -> (f64, f64) {
        let sign = if found.normal.get().dot(outward) > 0.0 { 1.0 } else { -1.0 };
        let (a, b) = (out.0 * sign, out.1 * sign);
        if a > b { (a, b) } else { (b, a) }
    }

    fn agree(found: &SurfaceLocal, want: (f64, f64)) -> bool {
        let (k1, k2) = found.curvatures;
        let forms = close(found.gaussian, want.0 * want.1) && close(found.mean, 0.5 * want.0 + 0.5 * want.1);
        close(k1, want.0) && close(k2, want.1) && forms
    }

    #[test]
    fn known_curvatures_for_every_analytic_surface_family() {
        let at = placement();
        let (origin, axis) = (local(0.0, 0.0, 0.0), local(0.0, 0.0, 1.0) - local(0.0, 0.0, 0.0));
        let radial = |p: Vec3| (p - origin) - axis * (p - origin).dot(axis);
        let plane = on_surface(&Plane::placed(&at), UvPoint::new(1.0, -2.0));
        assert!(agree(&plane, (0.0, 0.0)) && plane.directions.is_none(), "{plane:?}");
        let cylinder = on_surface(&Cylinder::placed(&at, mm(6.0)).unwrap(), UvPoint::new(0.7, 2.0));
        let want = signed((0.0, -1.0 / 6.0), radial(cylinder.point.to_vec3()), &cylinder);
        assert!(agree(&cylinder, want), "{cylinder:?}");
        let [along, around] = cylinder.directions.unwrap();
        assert!(
            along.get().cross(axis).norm() < 1e-9 && around.get().dot(axis).abs() < 1e-9,
            "the axis, then round it"
        );
        let sphere = on_surface(&Sphere::placed(&at, mm(5.0)).unwrap(), UvPoint::new(2.0, 0.4));
        let want = signed((-0.2, -0.2), sphere.point.to_vec3() - origin, &sphere);
        assert!(agree(&sphere, want) && sphere.directions.is_none(), "{sphere:?}");
        let (sin, cos) = occam_math::sin_cos(0.4);
        let cone = on_surface(&Cone::placed(&at, mm(3.0), Angle::from_radians(0.4)).unwrap(), UvPoint::new(1.0, 2.0));
        let want = signed((0.0, -cos / (3.0 + 2.0 * sin)), radial(cone.point.to_vec3()), &cone);
        assert!(agree(&cone, want), "{cone:?}");
        let torus = Torus::placed(&at, mm(8.0), mm(2.0)).unwrap();
        for v in [0.0, 1.0, PI] {
            let found = on_surface(&torus, UvPoint::new(0.5, v));
            let across = radial(found.point.to_vec3());
            let tube = found.point.to_vec3() - origin - across * (8.0 / across.norm());
            let want = signed((-0.5, -occam_math::cos(v) / (8.0 + 2.0 * occam_math::cos(v))), tube, &found);
            assert!(agree(&found, want), "{v}: {found:?} against {want:?}");
        }
    }

    #[test]
    fn a_surface_with_skewed_parameters_has_the_curvatures_and_directions_of_its_forms() {
        let s = forms::skewed();
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + b.abs());
        for uv in forms::places() {
            let found = on_surface(&s, uv);
            let (k1, k2) = forms::principal(&s, uv);
            assert!(near(found.curvatures.0, k1) && near(found.curvatures.1, k2), "{uv:?}: {found:?}, not {k1} {k2}");
            assert!(near(found.mean, 0.5 * k1 + 0.5 * k2) && near(found.gaussian, k1 * k2), "{uv:?}: {found:?}");
            let normal = forms::unit_normal(&s, uv);
            let [d1, d2] = found.directions.unwrap();
            for (d, k) in [(d1.get(), k1), (d2.get(), k2)] {
                assert!(d.dot(normal).abs() < 1e-12, "{uv:?}: {d:?} leaves the tangent plane");
                assert!(near(forms::normal_curvature(&s, uv, d), k), "{uv:?}: {d:?} is not the direction of {k}");
            }
            assert!(d1.get().dot(d2.get()).abs() < 1e-9, "{uv:?}: principal directions are perpendicular");
        }
    }

    #[test]
    fn derivatives_are_parallel_by_their_angle_and_span_no_plane_where_one_rounds_away() {
        // |a x b| is 4 against |a| |b| of 4 sqrt 2: within an angle whose sine is 0.8, not one whose sine is 0.5.
        let (a, b) = (Vec3::new(4.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0));
        assert!(parallel(a, b, 0.8) && !parallel(a, b, 0.5));
        // Square to each other, but the shorter rounds away against the longer, whichever is first.
        let (long, short) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1e-16, 0.0));
        assert!(no_plane(long, short, 1e-9) && no_plane(short, long, 1e-9));
        let longer = Vec3::new(1000.0, 0.0, 0.0);
        assert!(no_plane(longer, Vec3::new(0.0, 1e-12, 0.0), 1e-9), "64 epsilons of a thousand");
        assert!(!no_plane(longer, Vec3::new(0.0, 1e-9, 0.0), 1e-9));
    }

    #[test]
    fn of_two_rows_as_well_conditioned_the_first_gives_the_direction() {
        // The saddle (u, v, uv) at its centre bends by 1 along (1, 1, 0) and by -1 along (1, -1, 0). For the first,
        // `(II - k I) w = 0` has the rows (1, 1) and (-1, -1), as long as each other: the first is taken, so the
        // direction is (1, 1, 0) and not its opposite.
        let corners = vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0), point(0.0, 1.0, 0.0), point(1.0, 1.0, 1.0)];
        let saddle = on_surface(&SplineSurface::bezier((2, 2), corners, None).unwrap(), UvPoint::new(0.0, 0.0));
        assert_eq!(saddle.curvatures, (1.0, -1.0));
        let [first, second] = saddle.directions.unwrap();
        let half = 0.5_f64.sqrt();
        assert!(to_rounding(first.get(), Vec3::new(half, half, 0.0)), "{first:?}");
        assert!(to_rounding(second.get(), Vec3::new(half, -half, 0.0)), "{second:?}");
    }

    fn to_rounding(a: Vec3, b: Vec3) -> bool {
        (a - b).norm() < 1e-15
    }

    #[test]
    fn a_singular_point_reports_the_sine_between_its_derivatives() {
        // A bilinear patch whose derivatives at its first corner are (2, 0, 0) and (3, 6e-12, 0), of sine 2e-12.
        let corners = vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0), point(3.0, 6e-12, 0.0), point(5.0, 1.0, 1.0)];
        let patch = SplineSurface::bezier((2, 2), corners, None).unwrap();
        let report = surface_local(&patch, UvPoint::new(0.0, 0.0), tolerance(), &mut Scratch::default()).unwrap_err();
        let span = report.value("span").and_then(NamedValue::number);
        assert!(span.is_some_and(|s| (s - 2e-12).abs() < 1e-24), "{report:?}");
    }

    #[test]
    fn principal_directions_are_given_where_the_curvatures_differ_by_more_than_the_angle_of_the_larger() {
        // A circle of radius 2 drawn along -z, so that its normal points in: curvatures of 1/2 round it and none along
        // it, half of whose difference, 1/4, is more than 0.3 of the larger and no more than 0.6 of it.
        let ring = Circle::placed(&placement(), mm(2.0)).unwrap();
        let tube =
            Extrusion::new(ring, placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, -1.0)))).unwrap();
        let found = |angle: f64| {
            let tol = Tolerance::new(mm(1e-7), Angle::from_radians(angle)).unwrap();
            surface_local(&tube, UvPoint::new(0.4, 1.0), tol, &mut Scratch::default()).unwrap()
        };
        let fine = found(0.3);
        assert!(close(fine.curvatures.0, 0.5) && close(fine.curvatures.1, 0.0), "{fine:?}");
        assert!(fine.directions.is_some() && found(0.6).directions.is_none());
    }

    #[test]
    fn a_direction_that_does_not_exist_is_reported() {
        let (tol, mut scratch) = (tolerance(), Scratch::default());
        let at = placement();
        let sphere = Sphere::placed(&at, mm(5.0)).unwrap();
        let pole = surface_local(&sphere, UvPoint::new(1.0, FRAC_PI_2), tol, &mut scratch).unwrap_err();
        assert_eq!(pole.code, Code::SingularPoint, "a pole");
        let cone = Cone::placed(&at, mm(3.0), Angle::from_radians(0.4)).unwrap();
        let apex = -3.0 / occam_math::sin_cos(0.4).0;
        assert_eq!(
            surface_local(&cone, UvPoint::new(1.0, apex), tol, &mut scratch).unwrap_err().code,
            Code::SingularPoint
        );
        let corner = point(1.0, 1.0, 0.0);
        let stalled = SplineCurve::bezier(vec![corner, corner, point(3.0, 0.0, 0.0)], None).unwrap();
        let report = curve_local(&stalled, 0.0, tol, &mut scratch).unwrap_err();
        assert_eq!((report.code, report.value("span").and_then(NamedValue::number)), (Code::SingularPoint, Some(0.0)));
        let arc = Circle::placed(&at, mm(1.0)).unwrap().trimmed(0.5, 1.0).unwrap();
        assert_eq!(curve_local(&arc, 2.0, tol, &mut scratch).unwrap_err().code, Code::UndefinedParameter);
    }
}
