//! Source: original
//! Joining a curve in a face's parameters to its edge's curve in space (PARITY 2.3). They agree when the surface over
//! the one runs along the other within the band, the same way: their starts meet, their ends meet, and every point of
//! each, at evenly spaced samples, has a foot on the other no further than the band. The two may be parameterised
//! differently, so each is searched along for the other's samples, never compared at equal parameters.
//!
//! A parameter curve may cross a seam of the face's parameters, as an arc on a cylinder whose `u` runs past 2π does: the
//! surface is evaluated where the curve says, and a parameter that comes round is defined at every finite value, so the
//! surface's point there is the one it names. The check compares points in space, where a seam is no edge.

use occam_report::{Code, Fault, NamedValue, Report};
use occam_units::{Length, Tolerance, Vec3};

use crate::curve::Curve;
use crate::domain::ParamRange;
use crate::nearest::nearest_by;
use crate::param_curve::ParamCurve;
use crate::surface::Surface;

/// How many samples, ends apart, each curve is checked at.
const SAMPLES: u32 = 32;

/// The largest gap found so far, with the point of the edge's curve it is at and its parameter there.
#[derive(Clone, Copy)]
struct Worst {
    gap: f64,
    at: f64,
    point: Vec3,
}

impl Worst {
    /// This or `other`, whichever is the larger gap; the earlier of two as large.
    fn or(self, other: Self) -> Self {
        if other.gap > self.gap { other } else { self }
    }
}

/// The value a fraction `f` of the way from `lo` to `hi`.
fn lerp(lo: f64, hi: f64, f: f64) -> f64 {
    lo + (hi - lo) * f
}

/// The fractions of the way across a range at which the curves are sampled, the ends apart.
fn fractions() -> impl Iterator<Item = f64> {
    (1..SAMPLES).map(|k| f64::from(k) / f64::from(SAMPLES))
}

/// The point of `surface` over `pcurve` at `t`, and its first two derivatives in `t`, by the chain rule.
fn face_jet(surface: &dyn Surface, pcurve: &dyn ParamCurve, t: f64) -> [Vec3; 3] {
    let (uv, d1, d2) = pcurve.second(t);
    let jet = surface.second(uv);
    let (su, sv) = (jet.first.du.rate(), jet.first.dv.rate());
    let first = su * d1.u() + sv * d1.v();
    let curving = jet.duu.rate() * (d1.u() * d1.u())
        + jet.duv.rate() * (2.0 * d1.u() * d1.v())
        + jet.dvv.rate() * (d1.v() * d1.v());
    [jet.first.point.to_vec3(), first, curving + su * d2.u() + sv * d2.v()]
}

/// The largest gap between the surface over the parameter curve from `face.lo` to `face.hi`, and the edge's curve from
/// `edge.lo` to `edge.hi`: at their ends, then from samples of each to its foot on the other.
fn largest(
    surface: &dyn Surface,
    pcurve: &dyn ParamCurve,
    curve: &dyn Curve,
    (face, edge): (ParamRange, ParamRange),
) -> Worst {
    let on_face = |t: f64| surface.point(pcurve.uv(t)).to_vec3();
    let face_jet = |t: f64| face_jet(surface, pcurve, t);
    let edge_jet = |s: f64| {
        let (point, first, second) = curve.second(s);
        [point.to_vec3(), first.rate(), second.rate()]
    };
    let gap = |a: Vec3, b: Vec3| (a - b).norm();
    let ends = [(face.lo, edge.lo), (face.hi, edge.hi)].map(|(t, s)| {
        let point = curve.point(s).to_vec3();
        Worst { gap: gap(on_face(t), point), at: s, point }
    });
    let to_edge = fractions().map(|f| {
        let q = on_face(lerp(face.lo, face.hi, f));
        let s = nearest_by(edge_jet, q, (edge.lo, edge.hi));
        let point = curve.point(s).to_vec3();
        Worst { gap: gap(q, point), at: s, point }
    });
    let to_face = fractions().map(|f| {
        let s = lerp(edge.lo, edge.hi, f);
        let point = curve.point(s).to_vec3();
        let t = nearest_by(face_jet, point, (face.lo, face.hi));
        Worst { gap: gap(on_face(t), point), at: s, point }
    });
    let first = Worst { gap: 0.0, at: edge.lo, point: curve.point(edge.lo).to_vec3() };
    ends.into_iter().chain(to_edge).chain(to_face).fold(first, Worst::or)
}

/// Joins the curve `pcurve` in the parameters of `surface`, from `along_face.0` to `along_face.1`, to its edge's curve
/// `curve`, from `along_edge.0` to `along_edge.1`: the largest gap between them, which is no more than the band of `tol`.
///
/// # Errors
///
/// `geom.undefined-parameter` for a part that is not finite, leaves its curve's range, is empty or is longer than a
/// period; `geom.edge-off-surface` where the curves are further apart than the band anywhere, or run opposite ways,
/// naming the largest `gap`, the `point` of the edge's curve it is at and its `parameter` there, and the `band`.
pub fn join_edge(
    surface: &dyn Surface,
    (pcurve, along_face): (&dyn ParamCurve, (f64, f64)),
    (curve, along_edge): (&dyn Curve, (f64, f64)),
    tol: Tolerance,
) -> Result<Length, Report> {
    let face = pcurve.range().trimmed(along_face.0, along_face.1)?;
    let edge = curve.range().trimmed(along_edge.0, along_edge.1)?;
    let (worst, band) = (largest(surface, pcurve, curve, (face, edge)), tol.band().millimetres());
    let off = worst.gap > band || worst.gap.is_nan();
    if !off {
        return Ok(Length::from_millimetres(worst.gap));
    }
    let point = worst.point;
    Err(Report::new(Code::EdgeOffSurface, Fault::InvalidInput)
        .with_value(NamedValue::length("gap", worst.gap))
        .with_value(NamedValue::point("point", [point.x, point.y, point.z]))
        .with_value(NamedValue::new("parameter", edge.quantity(), worst.at))
        .with_value(NamedValue::length("band", band)))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use occam_units::{Angle, Direction, Local, Placement, Point, UvPoint, Vector, World};

    use super::*;
    use crate::conformance::{placement, tolerance};
    use crate::{
        BsplineCurve, Circle, Cone, Cylinder, Extrusion, Line, MeshSurface, OffsetSurface, Plane, Revolution, Sphere,
        SplineCurve, SplineSurface, Torus, TrimmedSurface, UvBox, UvCurve,
    };

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn p(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    fn turn(axis: [f64; 3], angle: f64) -> Placement<Local, Local> {
        let [x, y, z] = axis;
        Placement::rotation(Direction::normalised(Vec3::new(x, y, z)), Angle::from_radians(angle))
    }

    fn shift(x: f64, y: f64, z: f64) -> Placement<Local, Local> {
        Placement::translation(Vector::from_vec3(Vec3::new(x, y, z)))
    }

    /// [`placement`] after `local`.
    fn placed(local: Placement<Local, Local>) -> Placement<Local, World> {
        local.then(&placement())
    }

    /// The parameter line through `from` along `along`, from 0 to `length`.
    fn uv_line(from: (f64, f64), along: (f64, f64), length: f64) -> UvCurve<Line> {
        let line = Line::new(p(from.0, from.1, 0.0), Direction::normalised(Vec3::new(along.0, along.1, 0.0)));
        UvCurve::new(line.trimmed(0.0, length).unwrap()).unwrap()
    }

    /// A circle in the parameters about `(u, v)`.
    fn uv_circle(u: f64, v: f64, radius: f64) -> UvCurve<Circle> {
        UvCurve::new(Circle::placed(&shift(u, v, 0.0).then(&Placement::identity()), mm(radius)).unwrap()).unwrap()
    }

    fn joins(surface: &dyn Surface, (pc, face): (&dyn ParamCurve, (f64, f64)), (c, edge): (&dyn Curve, (f64, f64))) {
        let gap = join_edge(surface, (pc, face), (c, edge), tolerance());
        assert!(gap.as_ref().is_ok_and(|g| g.millimetres() <= 1e-12), "{:?}: {gap:?}", surface.family());
    }

    #[test]
    fn a_parameter_curve_on_each_surface_family_agrees_with_its_edge_within_the_band() {
        let plane = Plane::placed(&placement());
        let ring = Circle::placed(&placed(shift(1.5, -0.5, 0.0)), mm(2.0)).unwrap();
        joins(&plane, (&uv_circle(1.5, -0.5, 2.0), (0.0, 6.0)), (&ring, (0.0, 6.0)));
        let cylinder = Cylinder::placed(&placement(), mm(6.0)).unwrap();
        let across_seam = Circle::placed(&placed(shift(0.0, 0.0, 2.0)), mm(6.0)).unwrap().trimmed(5.0, 7.5).unwrap();
        joins(&cylinder, (&uv_line((5.0, 2.0), (1.0, 0.0), 2.5), (0.0, 2.5)), (&across_seam, (5.0, 7.5)));
        let axis = Direction::normalised(cylinder.first(UvPoint::new(1.0, 0.0)).dv.rate());
        let ruling = Line::new(cylinder.point(UvPoint::new(1.0, -3.0)), axis);
        joins(&cylinder, (&uv_line((1.0, -3.0), (0.0, 1.0), 7.0), (0.0, 7.0)), (&ruling, (0.0, 7.0)));
        let cone = Cone::placed(&placement(), mm(3.0), Angle::from_radians(0.4)).unwrap();
        let down = Direction::normalised(cone.first(UvPoint::new(0.7, -2.0)).dv.rate());
        let cone_ruling = Line::new(cone.point(UvPoint::new(0.7, -1.0)), down);
        joins(&cone, (&uv_line((0.7, -2.0), (0.0, 1.0), 7.0), (1.0, 7.0)), (&cone_ruling, (0.0, 6.0)));
        let sphere = Sphere::placed(&placement(), mm(5.0)).unwrap();
        let upright = turn([1.0, 0.0, 0.0], FRAC_PI_2).then(&turn([0.0, 0.0, 1.0], 1.2));
        let meridian = Circle::placed(&placed(upright), mm(5.0)).unwrap();
        joins(&sphere, (&uv_line((1.2, -1.0), (0.0, 1.0), 2.0), (0.0, 2.0)), (&meridian, (-1.0, 1.0)));
        let torus = Torus::placed(&placement(), mm(8.0), mm(2.0)).unwrap();
        let (sin, cos) = occam_math::sin_cos(0.5);
        let parallel = Circle::placed(&placed(shift(0.0, 0.0, 2.0 * sin)), mm(8.0 + 2.0 * cos)).unwrap();
        joins(&torus, (&uv_line((1.0, 0.5), (1.0, 0.0), 3.0), (0.0, 3.0)), (&parallel, (1.0, 4.0)));
    }

    /// A rational Bézier patch of three by three poles, and its first row as a rational Bézier curve.
    fn patch() -> (SplineSurface, SplineCurve) {
        let poles = vec![
            p(0.0, 0.0, 0.0),
            p(1.0, 0.2, 0.5),
            p(2.0, 0.0, 0.0),
            p(0.0, 1.0, 0.3),
            p(1.0, 1.0, 1.0),
            p(2.0, 1.2, 0.2),
            p(0.0, 2.0, 0.0),
            p(1.0, 2.1, 0.4),
            p(2.0, 2.0, 0.1),
        ];
        let weights = vec![1.0, 0.5, 2.0, 1.0, 1.5, 1.0, 0.75, 1.0, 1.25];
        let row = SplineCurve::bezier(poles[..3].to_vec(), Some(weights[..3].to_vec())).unwrap();
        (SplineSurface::bezier((3, 3), poles, Some(weights)).unwrap(), row)
    }

    /// A mesh of four by one cells over `u` and `v` from 0 to 1, and its bottom row as a polyline.
    #[expect(clippy::arithmetic_side_effects, reason = "small indices in a test")]
    fn strip() -> (MeshSurface, SplineCurve) {
        let places: Vec<UvPoint> =
            (0..2).flat_map(|j| (0..5).map(move |i| UvPoint::new(f64::from(i) / 4.0, f64::from(j)))).collect();
        let points: Vec<_> = places.iter().map(|uv| p(3.0 * uv.u(), uv.v(), uv.u() * uv.u() + 0.3 * uv.v())).collect();
        let triangles: Vec<[u32; 3]> = (0..4_u32).flat_map(|i| [[i, i + 1, i + 6], [i, i + 6, i + 5]]).collect();
        let mesh = MeshSurface::new(&points, places, &triangles).unwrap();
        let knots = vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0];
        let bottom = BsplineCurve { degree: 1, knots, poles: points[..5].to_vec(), weights: None };
        (mesh, SplineCurve::new(&bottom).unwrap())
    }

    #[test]
    fn a_parameter_curve_on_each_derived_family_agrees_with_its_edge_within_the_band() {
        let (spline, row) = patch();
        joins(&spline, (&uv_line((0.0, 0.0), (1.0, 0.0), 1.0), (0.0, 1.0)), (&row, (0.0, 1.0)));
        let (mesh, bottom) = strip();
        joins(&mesh, (&uv_line((0.0, 0.0), (1.0, 0.0), 1.0), (0.0, 1.0)), (&bottom, (0.0, 1.0)));
        let profile =
            Line::new(p(4.0, 0.0, 1.0), Direction::normalised(Vec3::new(0.3, 0.2, 1.0))).trimmed(0.0, 3.0).unwrap();
        let revolution = Revolution::new(p(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(0.0, 0.0, 1.0)), profile);
        joins(&revolution, (&uv_line((0.0, 0.0), (0.0, 1.0), 3.0), (0.0, 3.0)), (&profile, (0.0, 3.0)));
        let ring = Circle::placed(&placement(), mm(2.0)).unwrap();
        let up = placement().direction(Direction::normalised(Vec3::new(0.2, 0.0, 1.0)));
        let extrusion = Extrusion::new(ring, up).unwrap();
        let rise = Line::new(Curve::point(&ring, 0.5), up);
        joins(&extrusion, (&uv_line((0.5, 0.0), (0.0, 1.0), 2.0), (0.0, 2.0)), (&rise, (0.0, 2.0)));
        let lifted = OffsetSurface::new(Plane::placed(&placement()), mm(0.5)).unwrap();
        let above = Circle::placed(&placed(shift(1.5, -0.5, 0.5)), mm(2.0)).unwrap();
        joins(&lifted, (&uv_circle(1.5, -0.5, 2.0), (0.0, 6.0)), (&above, (0.0, 6.0)));
        let window = UvBox::new(UvPoint::new(0.0, -1.0), UvPoint::new(3.0, 3.0));
        let trimmed = TrimmedSurface::new(Cylinder::placed(&placement(), mm(6.0)).unwrap(), window).unwrap();
        let arc = Circle::placed(&placed(shift(0.0, 0.0, 2.0)), mm(6.0)).unwrap().trimmed(0.5, 2.5).unwrap();
        joins(&trimmed, (&uv_line((0.5, 2.0), (1.0, 0.0), 2.0), (0.0, 2.0)), (&arc, (0.5, 2.5)));
    }

    #[test]
    fn a_parameter_curve_off_its_edge_is_reported_with_the_largest_gap_and_where() {
        let cylinder = Cylinder::placed(&placement(), mm(6.0)).unwrap();
        let circle = Circle::placed(&placed(shift(0.0, 0.0, 2.0)), mm(6.0)).unwrap().trimmed(5.0, 7.5).unwrap();
        let higher = uv_line((5.0, 2.001), (1.0, 0.0), 2.5);
        let report = join_edge(&cylinder, (&higher, (0.0, 2.5)), (&circle, (5.0, 7.5)), tolerance()).unwrap_err();
        let named = |name: &str| report.value(name).and_then(NamedValue::number);
        assert_eq!((report.code, report.fault), (Code::EdgeOffSurface, Some(Fault::InvalidInput)));
        assert!(named("gap").is_some_and(|g| (g - 1e-3).abs() < 1e-9), "{report:?}");
        assert!(named("parameter").is_some_and(|t| (5.0..=7.5).contains(&t)), "{report:?}");
        assert_eq!(named("band"), Some(tolerance().band().millimetres()));
        assert!(report.value("point").is_some());
        // Run the other way, from 7.5 down to 5: the starts are a chord of 2.5 radians apart, 12 sin 1.25.
        let backwards = uv_line((7.5, 2.0), (-1.0, 0.0), 2.5);
        let report = join_edge(&cylinder, (&backwards, (0.0, 2.5)), (&circle, (5.0, 7.5)), tolerance()).unwrap_err();
        let gap = report.value("gap").and_then(NamedValue::number).unwrap();
        assert!((gap - 12.0 * occam_math::sin(1.25)).abs() < 1e-9, "{gap}");
        let at = report.value("parameter").and_then(NamedValue::number);
        assert!(at.is_some_and(|t| t.to_bits() == 5f64.to_bits() || t.to_bits() == 7.5f64.to_bits()), "an end: {at:?}");
    }

    #[test]
    fn the_surface_over_a_parameter_curve_has_the_derivatives_of_the_chain_rule() {
        // A circle in the parameters of a torus, where every term of the chain rule is some size: against central
        // differences of the point, whose error is some 1e-8 of the second derivative here.
        let torus = Torus::placed(&placement(), mm(8.0), mm(2.0)).unwrap();
        let ring = uv_circle(1.0, 0.5, 0.3);
        let on = |t: f64| torus.point(ring.uv(t)).to_vec3();
        for t in [0.3, 1.7, 4.0] {
            let [point, first, second] = face_jet(&torus, &ring, t);
            let h = 1e-4;
            let (ahead, behind) = (on(t + h), on(t - h));
            let slope = (ahead - behind) * (0.5 / h);
            let bend = (ahead - point * 2.0 + behind) * (1.0 / (h * h));
            assert!((point - on(t)).norm() < 1e-12, "{t}");
            assert!((first - slope).norm() < 1e-6 * slope.norm(), "{t}: {first:?} against {slope:?}");
            assert!((second - bend).norm() < 1e-5 * bend.norm(), "{t}: {second:?} against {bend:?}");
        }
    }

    /// A plane in the world's own axes, so its points are exactly their parameters, and the line along `x` from 0 to
    /// `length` as an edge.
    fn flat(length: f64) -> (Plane, Line) {
        let edge = Line::new(p(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        (Plane::placed(&Placement::identity()), edge.trimmed(0.0, length).unwrap())
    }

    #[test]
    fn every_sample_is_checked_and_the_largest_gap_is_where_it_is() {
        // A quadratic in the parameters meeting the edge at its ends and rising to a half at its middle, over the
        // edge's point at 2: the samples between the ends find it.
        let (plane, edge) = flat(4.0);
        let poles = vec![p(0.0, 0.0, 0.0), p(2.0, 1.0, 0.0), p(4.0, 0.0, 0.0)];
        let arch = UvCurve::new(SplineCurve::bezier(poles, None).unwrap()).unwrap();
        let report = join_edge(&plane, (&arch, (0.0, 1.0)), (&edge, (0.0, 4.0)), tolerance()).unwrap_err();
        let named = |name: &str| report.value(name).and_then(NamedValue::number);
        assert!(named("gap").is_some_and(|g| (g - 0.5).abs() < 1e-12), "{report:?}");
        assert!(named("parameter").is_some_and(|s| (s - 2.0).abs() < 1e-9), "{report:?}");
    }

    #[test]
    fn of_gaps_exactly_as_large_the_first_is_named_and_a_gap_of_the_band_is_within_it() {
        // A parameter line a thousandth above the edge: every gap is a thousandth to the last bit, so the first found,
        // at the edge's start, is the one named; and a band of exactly a thousandth holds it.
        let (plane, edge) = flat(3.0);
        let above = uv_line((0.0, 1e-3), (1.0, 0.0), 3.0);
        let report = join_edge(&plane, (&above, (0.0, 3.0)), (&edge, (0.0, 3.0)), tolerance()).unwrap_err();
        let named = |name: &str| report.value(name).and_then(NamedValue::number);
        assert_eq!((named("gap"), named("parameter")), (Some(1e-3), Some(0.0)), "{report:?}");
        let band = |b: f64| Tolerance::new(mm(b), tolerance().angle()).unwrap();
        assert_eq!(join_edge(&plane, (&above, (0.0, 3.0)), (&edge, (0.0, 3.0)), band(1e-3)), Ok(mm(1e-3)));
        let wider = join_edge(&plane, (&above, (0.0, 3.0)), (&edge, (0.0, 3.0)), band(0.999e-3));
        assert_eq!(wider.unwrap_err().code, Code::EdgeOffSurface);
    }

    #[test]
    fn a_part_outside_either_curve_is_refused() {
        let cylinder = Cylinder::placed(&placement(), mm(6.0)).unwrap();
        let whole = Circle::placed(&placed(shift(0.0, 0.0, 2.0)), mm(6.0)).unwrap();
        let along = uv_line((0.0, 2.0), (1.0, 0.0), 6.0);
        let code = |face: (f64, f64), edge: (f64, f64)| {
            join_edge(&cylinder, (&along, face), (&whole, edge), tolerance()).map(|_| ()).unwrap_err().code
        };
        assert_eq!(code((-1.0, 6.0), (0.0, 6.0)), Code::UndefinedParameter, "before the parameter curve");
        assert_eq!(code((0.0, 6.0), (0.0, 7.0)), Code::UndefinedParameter, "more than a turn of the edge");
        assert_eq!(code((3.0, 2.0), (0.0, 6.0)), Code::UndefinedParameter, "back to front");
        assert!(join_edge(&cylinder, (&along, (0.0, 6.0)), (&whole, (0.0, 6.0)), tolerance()).is_ok());
    }
}
