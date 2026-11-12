//! Source: original
//! How smoothly two curves, or two surfaces along an edge, join (PARITY 2.4), and a join that is less smooth than it
//! must be, reported. A curve's class is parametric, C0 to Cn, the derivatives at the end of one equal to those at the
//! start of the other, and geometric, G1 and G2, the unit tangents and then the curvature vectors equal. A surface's is
//! geometric along a shared edge, sampled across it: the points meet, then the tangent planes, then the normal
//! curvatures across the edge, which with the tangent planes equal along it make the curvatures equal in every
//! direction. A surface's parametric class depends on how the two are parameterised, which a shell does not fix, so
//! it is not measured.
//!
//! Points compare within the model's band. Every other comparison is relative, within the model's angle: two vectors
//! are equal where their difference is no longer than the angle times the longer, so the measure keeps its meaning
//! whatever the parameters measure. A derivative that is exactly zero on one side and rounding away from zero on the
//! other counts as different: a class is never reported higher than it can be shown.

use occam_math::atan2;
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Tangent, Tolerance, UvPoint, Vec3, World};

use crate::batch::Scratch;
use crate::curve::Curve;
use crate::local::{no_plane, singular};
use crate::surface::Surface;

/// A continuity class to require of a join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuity {
    /// Parametric of an order: `C(0)`, the points meet, to `C(n)`, the derivatives to the `n`-th equal too.
    C(u8),
    /// The points meet and the unit tangents are equal.
    G1,
    /// The unit tangents and the curvature vectors are equal.
    G2,
}

/// The classes a join was found to have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Smoothness {
    /// The highest order `n` to which it is `C(n)`, as far as was asked: `None` where the points do not meet.
    pub parametric: Option<u8>,
    /// The highest of 0, 1 and 2 to which it is geometrically continuous, 0 being the points meeting: `None` where
    /// they do not.
    pub geometric: Option<u8>,
}

impl Smoothness {
    /// Whether it has the class `required`.
    #[must_use]
    pub fn meets(self, required: Continuity) -> bool {
        match required {
            Continuity::C(n) => self.parametric.is_some_and(|found| found >= n),
            Continuity::G1 => self.geometric.is_some_and(|found| found >= 1),
            Continuity::G2 => self.geometric.is_some_and(|found| found >= 2),
        }
    }
}

/// What failed first at a join, and by how much: the order and whether it is geometric, and the measure with its
/// quantity.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Miss {
    order: u8,
    geometric: bool,
    gap: (f64, Quantity),
}

/// Whether `a` and `b` are the same vector within the relative angle `theta`.
fn same(a: Vec3, b: Vec3, theta: f64) -> bool {
    let (la, lb) = (a.norm(), b.norm());
    (a - b).norm() <= theta * if lb > la { lb } else { la }
}

/// Whether `a` and `b` are the same number within the relative angle `theta`.
fn same_number(a: f64, b: f64, theta: f64) -> bool {
    (a - b).abs() <= theta * if b.abs() > a.abs() { b.abs() } else { a.abs() }
}

/// The angle between `a` and `b`.
fn angle(a: Vec3, b: Vec3) -> f64 {
    atan2(a.cross(b).norm(), a.dot(b))
}

/// The curvature vector of derivatives `first` and `second`: the second's part across the first, over the speed squared.
fn bend(first: Vec3, second: Vec3) -> Vec3 {
    let speed = first.dot(first);
    (second - first * (second.dot(first) / speed)) * (1.0 / speed)
}

/// The derivatives of `c` at `t`, the point first, to `order`.
fn jets(c: &dyn Curve, t: f64, order: u8, scratch: &mut Scratch) -> Result<Vec<Vec3>, Report> {
    let mut out: Vec<Tangent<World>> = Vec::new();
    c.derivatives(t, order, &mut out, scratch)?;
    Ok(std::iter::once(c.point(t).to_vec3()).chain(out.iter().map(|d| d.rate())).collect())
}

/// The join of `first`'s end to `second`'s start, measured to `up_to`, and what failed first in each kind.
fn measure(
    (first, second): (&dyn Curve, &dyn Curve),
    tol: Tolerance,
    up_to: u8,
    scratch: &mut Scratch,
) -> Result<(Smoothness, [Option<Miss>; 2]), Report> {
    let (end, start) = (first.range().hi, second.range().lo);
    let order = if up_to > 2 { up_to } else { 2 };
    let (a, b) = (jets(first, end, order, scratch)?, jets(second, start, order, scratch)?);
    let theta = tol.angle().radians();
    let at = |v: &[Vec3], k: usize| v.get(k).copied().unwrap_or(Vec3::new(f64::NAN, f64::NAN, f64::NAN));
    let gap = (at(&a, 0) - at(&b, 0)).norm();
    let meet = gap <= tol.band().millimetres();
    if !meet {
        let miss = Miss { order: 0, geometric: false, gap: (gap, Quantity::Length) };
        let none = Smoothness { parametric: None, geometric: None };
        return Ok((none, [Some(miss), Some(Miss { geometric: true, ..miss })]));
    }
    let parametric = (1..=up_to).find(|k| !same(at(&a, usize::from(*k)), at(&b, usize::from(*k)), theta));
    let parametric_miss = parametric.map(|k| {
        let difference = (at(&a, usize::from(k)) - at(&b, usize::from(k))).norm();
        Miss { order: k, geometric: false, gap: (difference, Quantity::Ratio) }
    });
    let reached = parametric.map_or(up_to, |k| k.saturating_sub(1));
    let (geometric, geometric_miss) = geometric_class((at(&a, 1), at(&a, 2)), (at(&b, 1), at(&b, 2)), theta);
    Ok((Smoothness { parametric: Some(reached), geometric: Some(geometric) }, [parametric_miss, geometric_miss]))
}

/// The geometric class, 0 to 2, of a join whose points meet, from the first and second derivatives on each side, and
/// what failed first.
fn geometric_class(a: (Vec3, Vec3), b: (Vec3, Vec3), theta: f64) -> (u8, Option<Miss>) {
    let turned = angle(a.0, b.0);
    let tangent = a.0.norm() > 0.0 && b.0.norm() > 0.0 && turned <= theta;
    if !tangent {
        return (0, Some(Miss { order: 1, geometric: true, gap: (turned, Quantity::Angle) }));
    }
    let (bend_a, bend_b) = (bend(a.0, a.1), bend(b.0, b.1));
    if same(bend_a, bend_b, theta) {
        return (2, None);
    }
    (1, Some(Miss { order: 2, geometric: true, gap: ((bend_a - bend_b).norm(), Quantity::Ratio) }))
}

/// The classes of the join of `first`'s end to `second`'s start, the parametric to at most `up_to`.
///
/// # Errors
///
/// `geom.undefined-parameter` where either end is not finite, and what either curve's derivatives report.
pub fn curve_join(
    first: &dyn Curve,
    second: &dyn Curve,
    tol: Tolerance,
    up_to: u8,
    scratch: &mut Scratch,
) -> Result<Smoothness, Report> {
    Ok(measure((first, second), tol, up_to, scratch)?.0)
}

/// The classes of the join of `first`'s end to `second`'s start, where it has the class `required`.
///
/// # Errors
///
/// `geom.join-below-continuity` where it does not, naming the first order that fails, the order required, and the
/// measure that fails it; and as [`curve_join`].
pub fn require_curve_join(
    first: &dyn Curve,
    second: &dyn Curve,
    required: Continuity,
    tol: Tolerance,
    scratch: &mut Scratch,
) -> Result<Smoothness, Report> {
    let up_to = match required {
        Continuity::C(n) => n,
        Continuity::G1 | Continuity::G2 => 0,
    };
    let (found, [parametric, geometric]) = measure((first, second), tol, up_to, scratch)?;
    if found.meets(required) {
        return Ok(found);
    }
    let (miss, order) = match required {
        Continuity::C(n) => (parametric, n),
        Continuity::G1 => (geometric, 1),
        Continuity::G2 => (geometric, 2),
    };
    Err(below(miss.unwrap_or(Miss { order, geometric: true, gap: (f64::NAN, Quantity::Ratio) }), order))
}

/// The refusal of a join whose first failure is `miss`, where `required` was required.
fn below(miss: Miss, required: u8) -> Report {
    let count = |name: &str, n: u8| NamedValue::new(name, Quantity::Count, f64::from(n));
    let (gap, quantity) = miss.gap;
    Report::new(Code::JoinBelowContinuity, Fault::InvalidInput)
        .with_value(count("failed", miss.order))
        .with_value(count("required", required))
        .with_value(NamedValue::new("gap", quantity, if gap.is_finite() { gap } else { 0.0 }))
        .with_value(count("geometric", u8::from(miss.geometric)))
}

/// The unit normal of `s` at `uv`, and its normal curvature across `along` towards that normal: the curvature is `None`
/// where `s` has no second derivatives and `second` is false, and their absence is an error where it is true. The
/// tangent across, `w`, is any length, and so are its coordinates `(du, dv)` in the derivatives' basis, which are taken
/// times the first form's determinant: the curvature is a ratio of two forms that are both quadratic in them.
#[expect(clippy::many_single_char_names, reason = "the fundamental forms' coefficients, as do Carmo names them")]
fn across(
    s: &dyn Surface,
    uv: UvPoint,
    along: Vec3,
    (theta, second): (f64, bool),
    scratch: &mut Scratch,
) -> Result<(Vec3, Option<f64>), Report> {
    let mut out: Vec<Tangent<World>> = Vec::new();
    let order = match s.derivatives(uv, 2, &mut out, scratch) {
        Ok(()) => 2,
        Err(report) if report.code == Code::NoDerivative && !second => {
            s.derivatives(uv, 1, &mut out, scratch)?;
            1
        }
        Err(report) => return Err(report),
    };
    let nan = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
    let rate = |k: usize| out.get(k).map_or(nan, |t| t.rate());
    let (su, sv) = (rate(0), rate(1));
    if no_plane(su, sv, theta) {
        return Err(singular(su.cross(sv).norm() / (su.norm() * sv.norm())));
    }
    let normal = su.cross(sv) * (1.0 / su.cross(sv).norm());
    if order < 2 {
        return Ok((normal, None));
    }
    let w = normal.cross(along);
    let (e, f, g) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let (wu, wv) = (w.dot(su), w.dot(sv));
    let (du, dv) = (g * wu - f * wv, e * wv - f * wu);
    let (l, m, n) = (rate(2).dot(normal), rate(3).dot(normal), rate(4).dot(normal));
    let second_form = l * du * du + 2.0 * m * du * dv + n * dv * dv;
    let first_form = e * du * du + 2.0 * f * du * dv + g * dv * dv;
    Ok((normal, Some(second_form / first_form)))
}

/// The geometric class, 0 to 2, of `first` and `second` joined along `edge` from `lo` to `hi`, sampled at nine points of
/// it, ends included, and what failed first: `None` where the points do not meet.
fn measure_surfaces(
    (first, second): (&dyn Surface, &dyn Surface),
    (edge, (lo, hi)): (&dyn Curve, (f64, f64)),
    (tol, need_second): (Tolerance, bool),
    scratch: &mut Scratch,
) -> Result<(Option<u8>, Option<Miss>), Report> {
    let (theta, band) = (tol.angle().radians(), tol.band().millimetres());
    let (mut apart, mut turned, mut bent, mut known) = (0.0_f64, 0.0_f64, 0.0_f64, true);
    for k in 0..=8_u32 {
        let t = lo + (hi - lo) * f64::from(k) / 8.0;
        let (q, along) = (edge.point(t), edge.first(t).1.rate());
        let (c1, c2) = (first.closest(q, None, tol, scratch)?, second.closest(q, None, tol, scratch)?);
        let far = if c2.distance > c1.distance { c2.distance } else { c1.distance };
        apart = if far.millimetres() > apart { far.millimetres() } else { apart };
        let (n1, k1) = across(first, c1.at, along, (theta, need_second), scratch)?;
        let (n2, k2) = across(second, c2.at, along, (theta, need_second), scratch)?;
        let flip = if n1.dot(n2) < 0.0 { -1.0 } else { 1.0 };
        let off = angle(n1, n2 * flip);
        turned = if off > turned { off } else { turned };
        match (k1, k2) {
            (Some(k1), Some(k2)) if !same_number(k1, k2 * flip, theta) => {
                let gap = (k1 - k2 * flip).abs();
                bent = if gap > bent { gap } else { bent };
            }
            (Some(_), Some(_)) => {}
            _ => known = false,
        }
    }
    let miss = |order: u8, gap: (f64, Quantity)| Some(Miss { order, geometric: true, gap });
    let (meet, tangent) = (apart <= band, turned <= theta);
    Ok(if !meet {
        (None, miss(0, (apart, Quantity::Length)))
    } else if !tangent {
        (Some(0), miss(1, (turned, Quantity::Angle)))
    } else if !known || bent > 0.0 {
        (Some(1), miss(2, (bent, Quantity::Ratio)))
    } else {
        (Some(2), None)
    })
}

/// The geometric class, 0 to 2, of `first` and `second` joined along `edge` from `lo` to `hi`: `None` where they do
/// not meet. A surface without second derivatives, as a mesh, is shown G1 at most.
///
/// # Errors
///
/// `geom.singular-point` where either surface has no normal at a sample, and what their feet and derivatives report.
pub fn surface_join(
    first: &dyn Surface,
    second: &dyn Surface,
    edge: (&dyn Curve, (f64, f64)),
    tol: Tolerance,
    scratch: &mut Scratch,
) -> Result<Option<u8>, Report> {
    Ok(measure_surfaces((first, second), edge, (tol, false), scratch)?.0)
}

/// As [`surface_join`], where the join has the class `required`: `C(0)`, `G1` or `G2`.
///
/// # Errors
///
/// `geom.join-below-continuity` where it does not, `geom.out-of-range` for a parametric class above `C(0)`, which a
/// surface's join is not measured in, `geom.no-derivative` for `G2` of a surface without second derivatives, and as
/// [`surface_join`].
pub fn require_surface_join(
    first: &dyn Surface,
    second: &dyn Surface,
    edge: (&dyn Curve, (f64, f64)),
    (required, tol): (Continuity, Tolerance),
    scratch: &mut Scratch,
) -> Result<Option<u8>, Report> {
    let order = match required {
        Continuity::C(0) => 0,
        Continuity::G1 => 1,
        Continuity::G2 => 2,
        Continuity::C(n) => {
            return Err(Report::new(Code::OutOfRange, Fault::InvalidInput)
                .with_value(NamedValue::new("value", Quantity::Count, f64::from(n)))
                .with_value(NamedValue::new("low", Quantity::Count, -1.0))
                .with_value(NamedValue::new("high", Quantity::Count, 1.0)));
        }
    };
    let (found, miss) = measure_surfaces((first, second), edge, (tol, order == 2), scratch)?;
    match (found, miss) {
        (Some(class), _) if class >= order => Ok(found),
        (_, Some(miss)) => Err(below(miss, order)),
        (_, None) => Ok(found),
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use occam_units::{Angle, Direction, Length, Local, Placement, Point, Vector};

    use super::*;
    use crate::conformance::{agree, forms, placement, tolerance};
    use crate::{
        BsplineCurve, BsplineSurface, Circle, Cylinder, Line, MeshSurface, Plane, Sphere, SplineCurve, SplineSurface,
    };

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    /// Turned about `axis` by `angle`, then moved to `origin`.
    fn placed(axis: Vec3, angle: f64, origin: Vec3) -> Placement<Local, World> {
        let turn = Placement::<Local, Local>::rotation(Direction::normalised(axis), Angle::from_radians(angle));
        turn.then(&Placement::identity()).then(&Placement::<World, World>::translation(Vector::from_vec3(origin)))
    }

    fn line(from: Vec3, along: Vec3, length: f64) -> Line {
        Line::new(Point::from_vec3(from), Direction::normalised(along)).trimmed(0.0, length).unwrap()
    }

    /// An arc of radius `r` about `centre` in the XY plane, from angle `lo` to `hi`.
    fn arc(centre: Vec3, r: f64, (lo, hi): (f64, f64)) -> Circle {
        Circle::placed(&placed(Vec3::new(0.0, 0.0, 1.0), 0.0, centre), mm(r)).unwrap().trimmed(lo, hi).unwrap()
    }

    fn join(a: &dyn Curve, b: &dyn Curve) -> Smoothness {
        curve_join(a, b, tolerance(), 4, &mut Scratch::default()).unwrap()
    }

    fn found(parametric: Option<u8>, geometric: Option<u8>) -> Smoothness {
        Smoothness { parametric, geometric }
    }

    /// A cubic B-spline with a simple knot at a half, cut there into its two parts.
    fn halves() -> (SplineCurve, SplineCurve) {
        let poles = [
            point(0.0, 0.0, 0.0),
            point(1.0, 2.0, 0.5),
            point(3.0, -1.0, 1.0),
            point(4.0, 2.0, 0.0),
            point(6.0, 0.0, 1.0),
        ];
        let knots = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
        let whole = SplineCurve::new(&BsplineCurve { degree: 3, knots, poles: poles.to_vec(), weights: None }).unwrap();
        let part = |lo, hi| SplineCurve::new(&Curve::to_bspline(&whole, lo, hi).unwrap()).unwrap();
        (part(0.0, 0.5), part(0.5, 1.0))
    }

    fn straight_then_round() -> (Line, Circle) {
        (
            line(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 5.0),
            arc(Vec3::new(0.0, 3.0, 0.0), 3.0, (-FRAC_PI_2, 0.0)),
        )
    }

    #[test]
    fn continuity_at_known_joins() {
        let (straight, round) = straight_then_round();
        assert_eq!(join(&straight, &round), found(Some(0), Some(1)), "tangent, at different speeds and curvatures");
        let circle = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(
            join(&arc(circle, 2.0, (0.0, 1.0)), &arc(circle, 2.0, (1.0, 2.0))),
            found(Some(4), Some(2)),
            "one circle"
        );
        let (first, second) = halves();
        assert_eq!(join(&first, &second), found(Some(2), Some(2)), "a cubic at a simple knot is C2 and no more");
        let small = arc(Vec3::new(0.0, 2.0, 0.0), 2.0, (-FRAC_PI_2 - 1.0, -FRAC_PI_2));
        let large = arc(Vec3::new(0.0, 5.0, 0.0), 5.0, (-FRAC_PI_2, -FRAC_PI_2 + 1.0));
        assert_eq!(join(&small, &large), found(Some(0), Some(1)), "tangent arcs of two radii");
        let lifted = arc(Vec3::new(0.0, 3.001, 0.0), 3.0, (-FRAC_PI_2, 0.0));
        assert_eq!(join(&straight, &lifted), found(None, None), "apart");
        let corner = line(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 3.0);
        assert_eq!(join(&straight, &corner), found(Some(0), Some(0)), "a corner");
    }

    fn refused(
        a: &dyn Curve,
        b: &dyn Curve,
        required: Continuity,
    ) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
        let report = require_curve_join(a, b, required, tolerance(), &mut Scratch::default()).unwrap_err();
        assert_eq!(report.code, Code::JoinBelowContinuity);
        let named = |name: &str| report.value(name).and_then(NamedValue::number);
        (named("failed"), named("required"), named("gap"), named("geometric"))
    }

    #[test]
    fn a_join_below_what_was_required_is_reported() {
        let (straight, round) = straight_then_round();
        let (failed, required, gap, geometric) = refused(&straight, &round, Continuity::G2);
        assert_eq!((failed, required, geometric), (Some(2.0), Some(2.0), Some(1.0)));
        assert!(gap.is_some_and(|g| (g - 1.0 / 3.0).abs() < 1e-12), "the curvature vectors differ by a third: {gap:?}");
        let (failed, required, gap, geometric) = refused(&straight, &round, Continuity::C(1));
        assert_eq!((failed, required, geometric), (Some(1.0), Some(1.0), Some(0.0)));
        assert!(gap.is_some_and(|g| (g - 2.0).abs() < 1e-12), "speeds of one and three: {gap:?}");
        let held = require_curve_join(&straight, &round, Continuity::G1, tolerance(), &mut Scratch::default());
        assert_eq!(held, Ok(found(Some(0), Some(1))));
        let lifted = arc(Vec3::new(0.0, 3.001, 0.0), 3.0, (-FRAC_PI_2, 0.0));
        let (failed, _, gap, _) = refused(&straight, &lifted, Continuity::C(0));
        assert!(failed == Some(0.0) && gap.is_some_and(|g| (g - 1e-3).abs() < 1e-12), "{gap:?}");
        let (first, second) = halves();
        assert_eq!(refused(&first, &second, Continuity::C(3)).0, Some(3.0));
        assert!(require_curve_join(&first, &second, Continuity::C(2), tolerance(), &mut Scratch::default()).is_ok());
    }

    #[test]
    fn vectors_and_numbers_are_the_same_within_the_angle_of_the_larger() {
        // 0.8 apart: within 0.5 of the larger, 1.8, though not of the smaller, 1; and not within 0.4 of either.
        let (a, b) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.8, 0.0, 0.0));
        assert!(same(a, b, 0.5) && same(b, a, 0.5) && !same(a, b, 0.4));
        assert!(same_number(1.0, 1.8, 0.5) && same_number(1.8, 1.0, 0.5) && !same_number(1.0, 1.8, 0.4));
        assert!(same_number(-1.0, -1.8, 0.5) && !same_number(1.0, -1.0, 0.5));
        // Moving at (2, 0, 0) and turning with (1, 1, 0): the part across, (0, 1, 0), over the speed squared.
        assert!(agree(bend(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0)), Vec3::new(0.0, 0.25, 0.0)));
    }

    #[test]
    fn a_side_that_stops_at_the_join_has_no_tangent_there() {
        // A cubic whose last two poles are one point stops at its end; a cubic whose first two are stops at its start.
        let stops = SplineCurve::bezier(
            vec![point(0.0, 0.0, 0.0), point(1.0, 1.0, 0.0), point(2.0, 0.0, 0.0), point(2.0, 0.0, 0.0)],
            None,
        )
        .unwrap();
        let onward = line(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0);
        assert_eq!(join(&stops, &onward), found(Some(0), Some(0)));
        let starts = SplineCurve::bezier(
            vec![point(2.0, 0.0, 0.0), point(2.0, 0.0, 0.0), point(3.0, 1.0, 0.0), point(4.0, 0.0, 0.0)],
            None,
        )
        .unwrap();
        let before = line(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0);
        assert_eq!(join(&before, &starts), found(Some(0), Some(0)));
    }

    #[test]
    fn a_refusal_names_the_class_it_measured_and_by_how_much() {
        // Apart, held to G1: the geometric class fails first, at order 0.
        let (straight, _) = straight_then_round();
        let lifted = arc(Vec3::new(0.0, 3.001, 0.0), 3.0, (-FRAC_PI_2, 0.0));
        assert_eq!(refused(&straight, &lifted, Continuity::G1).3, Some(1.0));
        // Tangent arcs of radii 2 and 5 curving the same way: curvature vectors of a half and a fifth, 0.3 apart.
        let small = arc(Vec3::new(0.0, 2.0, 0.0), 2.0, (-FRAC_PI_2 - 1.0, -FRAC_PI_2));
        let large = arc(Vec3::new(0.0, 5.0, 0.0), 5.0, (-FRAC_PI_2, -FRAC_PI_2 + 1.0));
        let (failed, _, gap, geometric) = refused(&small, &large, Continuity::G2);
        assert_eq!((failed, geometric), (Some(2.0), Some(1.0)));
        assert!(gap.is_some_and(|g| (g - 0.3).abs() < 1e-12), "{gap:?}");
    }

    fn along_x() -> Line {
        line(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 2.0)
    }

    fn surfaces(a: &dyn Surface, b: &dyn Surface) -> Option<u8> {
        let edge = along_x();
        surface_join(a, b, (&edge, (0.0, 2.0)), tolerance(), &mut Scratch::default()).unwrap()
    }

    fn require(a: &dyn Surface, b: &dyn Surface, required: Continuity) -> Result<Option<u8>, Report> {
        let edge = along_x();
        require_surface_join(a, b, (&edge, (0.0, 2.0)), (required, tolerance()), &mut Scratch::default())
    }

    /// A cylinder of radius `r` about a line along x at height `r`, touching the plane z = 0 along the x axis.
    fn roller(r: f64) -> Cylinder {
        Cylinder::placed(&placed(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2, Vec3::new(0.0, 0.0, r)), mm(r)).unwrap()
    }

    /// The plane z = 0 from x = -0.6 to 0.5, rising to a height of 1 by x = -2 and by x = 2: a surface of degree one.
    fn ramp() -> SplineSurface {
        let rises = [(-2.0, 1.0), (-0.6, 0.0), (0.5, 0.0), (2.0, 1.0)];
        let poles = [-1.0, 1.0].iter().flat_map(|y| rises.iter().map(move |(x, z)| point(*x, *y, *z))).collect();
        let third = 1.0 / 3.0;
        let knots_u = vec![0.0, 0.0, third, 2.0 * third, 1.0, 1.0];
        let form = BsplineSurface {
            degree: (1, 1),
            count: (4, 2),
            knots_u,
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            poles,
            weights: None,
        };
        SplineSurface::new(&form).unwrap()
    }

    #[test]
    fn a_join_is_sampled_along_the_edge_between_its_ends_and_nowhere_else() {
        // The edge's part from 0.5 to 1.4 runs from x = -0.5 to 0.4, where the ramp lies on the ground.
        let (ground, ramp, edge) = (Plane::placed(&Placement::identity()), ramp(), along_x());
        let join = |range| surface_join(&ground, &ramp, (&edge, range), tolerance(), &mut Scratch::default()).unwrap();
        assert_eq!(join((0.5, 1.4)), Some(2));
        assert_eq!(join((0.5, 2.0)), None, "past x = 0.5 the ramp rises");
    }

    #[test]
    fn curvatures_across_are_the_same_within_the_angle_and_differ_by_their_difference() {
        // Radii of 2 and 2 + 4e-13: curvatures across differing by some 1e-13 of themselves, which is within the angle.
        assert_eq!(surfaces(&roller(2.0), &roller(2.0 + 4e-13)), Some(2));
        // Radii of 2 and 5, both above the ground and bending the same way: a half and a fifth, 0.3 apart.
        let curved = require(&roller(2.0), &roller(5.0), Continuity::G2).unwrap_err();
        let gap = curved.value("gap").and_then(NamedValue::number);
        assert!(gap.is_some_and(|g| (g - 0.3).abs() < 1e-9), "{curved:?}");
    }

    #[test]
    fn the_curvature_across_an_edge_is_the_normal_curvature_square_to_it() {
        let skewed = forms::skewed();
        let alongs = [Vec3::new(3.0, 0.5, 0.2), Vec3::new(-0.4, 2.0, 1.0), Vec3::new(0.3, -0.2, 4.0)];
        for uv in forms::places() {
            let normal = forms::unit_normal(&skewed, uv);
            for along in alongs {
                let (n, k) = across(&skewed, uv, along, (1e-9, true), &mut Scratch::default()).unwrap();
                let want = forms::normal_curvature(&skewed, uv, normal.cross(along));
                assert!(agree(n, normal), "{uv:?}");
                assert!(
                    k.is_some_and(|k| (k - want).abs() <= 1e-9 * (1.0 + want.abs())),
                    "{uv:?} {along:?}: {k:?}, {want}"
                );
            }
        }
        // Every way across a sphere of radius a half bends by 2.
        let ball = Sphere::placed(&placement(), mm(0.5)).unwrap();
        for along in alongs {
            let (_, k) = across(&ball, UvPoint::new(1.0, 0.3), along, (1e-9, true), &mut Scratch::default()).unwrap();
            assert!(k.is_some_and(|k| (k.abs() - 2.0).abs() < 1e-12), "{along:?}: {k:?}");
        }
        // A corner whose derivatives are (2, 0, 0) and (3, 6e-12, 0), of sine 2e-12, has no normal.
        let corners = vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0), point(3.0, 6e-12, 0.0), point(5.0, 1.0, 1.0)];
        let patch = SplineSurface::bezier((2, 2), corners, None).unwrap();
        let corner = UvPoint::new(0.0, 0.0);
        let report = across(&patch, corner, alongs[0], (1e-9, false), &mut Scratch::default()).unwrap_err();
        let span = report.value("span").and_then(NamedValue::number);
        assert!(span.is_some_and(|s| (s - 2e-12).abs() < 1e-24), "{report:?}");
    }

    #[test]
    fn surfaces_join_along_an_edge_as_their_tangent_planes_and_curvatures_across_it_do() {
        let ground = Plane::placed(&Placement::identity());
        // A cylinder of radius 2 about the line through (0, 0, 2) along x, touching the plane along the x axis.
        let roller =
            Cylinder::placed(&placed(Vec3::new(0.0, 1.0, 0.0), FRAC_PI_2, Vec3::new(0.0, 0.0, 2.0)), mm(2.0)).unwrap();
        assert_eq!(surfaces(&ground, &roller), Some(1), "tangent planes meet, curvatures across do not");
        assert_eq!(surfaces(&roller, &roller), Some(2));
        let folded = Plane::placed(&placed(Vec3::new(1.0, 0.0, 0.0), 0.5, Vec3::new(0.0, 0.0, 0.0)));
        assert_eq!(surfaces(&ground, &folded), Some(0), "a crease");
        let raised = Plane::placed(&placed(Vec3::new(0.0, 0.0, 1.0), 0.0, Vec3::new(0.0, 0.0, 1e-3)));
        assert_eq!(surfaces(&ground, &raised), None, "a step");
        assert_eq!(require(&ground, &roller, Continuity::G1), Ok(Some(1)));
        let curved = require(&ground, &roller, Continuity::G2).unwrap_err();
        let named = |name: &str| curved.value(name).and_then(NamedValue::number);
        assert_eq!(
            (curved.code, named("failed"), named("required")),
            (Code::JoinBelowContinuity, Some(2.0), Some(2.0))
        );
        assert!(named("gap").is_some_and(|g| (g - 0.5).abs() < 1e-9), "{:?}", named("gap"));
        assert_eq!(
            require(&ground, &folded, Continuity::G1).unwrap_err().value("failed").and_then(NamedValue::number),
            Some(1.0)
        );
        assert_eq!(require(&ground, &raised, Continuity::C(0)).unwrap_err().code, Code::JoinBelowContinuity);
        let beyond = require(&ground, &roller, Continuity::C(1)).unwrap_err();
        let value = |name: &str| beyond.value(name).and_then(NamedValue::number);
        assert_eq!(beyond.code, Code::OutOfRange);
        assert_eq!((value("value"), value("low"), value("high")), (Some(1.0), Some(-1.0), Some(1.0)));
    }

    #[test]
    fn a_mesh_joins_to_g1_at_most_and_cannot_be_held_to_g2() {
        let corners = [point(-2.0, -2.0, 0.0), point(2.0, -2.0, 0.0), point(2.0, 2.0, 0.0), point(-2.0, 2.0, 0.0)];
        let places = corners.iter().map(|p| occam_units::UvPoint::new(p.to_vec3().x, p.to_vec3().y)).collect();
        let mesh = MeshSurface::new(&corners, places, &[[0, 1, 2], [0, 2, 3]]).unwrap();
        let ground = Plane::placed(&Placement::identity());
        assert_eq!(surfaces(&ground, &mesh), Some(1));
        assert_eq!(require(&ground, &mesh, Continuity::G2).unwrap_err().code, Code::NoDerivative);
    }
}
