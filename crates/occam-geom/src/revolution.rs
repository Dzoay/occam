//! Source: original
//! The surface of revolution: a curve turned about an axis (PARITY 2.2), `origin + h z + a e(u) + b e'(u)`, where
//! `(a, b, h)` are the curve's coordinates along the axes from the origin at `v`, `e(u) = cos u x + sin u y`, and `u` is
//! the angle turned, coming round, so that `u = 0` is the curve itself. The curve need not lie in a plane through the
//! axis. The partial derivative `i` times in `u` and `j` in `v` turns the curve's `j`-th derivative across the axis by
//! `i` quarter turns, and keeps its part along the axis only where `i` is 0: one formula for every order, as
//! meridian.rs has for the analytic surfaces turned about an axis. The curve is held by type, so its rungs run in
//! lanes, and may run without end. Its foot is the meridian's foot, then the angle between.

use occam_math::{Real, atan2, sin_cos};
use occam_report::Report;
use occam_units::{Bounds, Direction, Length, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::axes::Axes;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::BsplineSurface;
use crate::curve::ParametricCurve;
use crate::domain::{Degeneracy, Domain, Param, ParamRange, UvBox};
use crate::exact;
use crate::identity::{Content, FamilyName, Shape};
use crate::nearest::nearest_by;
use crate::support::{everything, splat};
use crate::surface::Parametric;
use crate::swept::{corners, finite, ordered, push_curve, window, with_curve};
use crate::turn::{above, quarter_turns, turn};

/// A curve turned about an axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Revolution<C> {
    axes: Axes,
    curve: C,
    range: ParamRange,
    /// The ends of the curve that lie on the axis, where a line of constant `v` collapses to a point.
    poles: Vec<Degeneracy>,
}

/// The partial derivative `i` times in `u` and `j` in `v`, given `(cos u, sin u)` and `w`, the curve's `j`-th
/// derivative, or for `j` = 0 its point less the origin: `a e(i) + b e(i + 1)`, and `h z` where `i` is 0.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn partial<R: Real>(axes: &Axes, (cos, sin): (R, R), w: Vec3<R>, i: u8) -> Vec3<R> {
    let up = splat(axes.normal.get());
    let (a, b) = (w.dot(splat(axes.x)), w.dot(splat(axes.y)));
    let ((ex, ey), (fx, fy)) = (quarter_turns(cos, sin, i), quarter_turns(cos, sin, i.saturating_add(1)));
    let across = axes.along(a * ex + b * fx, a * ey + b * fy);
    if i == 0 { across + up * w.dot(up) } else { across }
}

/// The curve's end at `t` as a degenerate line, if it lies on the axis within the rounding of the numbers there: a
/// collapsed line, and where the curve leaves the axis square to it, the normal, along the axis away from the curve's
/// other end (`side` is -1 at the low end and 1 at the high). Elsewhere the curve may cross the axis, and nothing is
/// declared there rather than guessed.
fn collapse<C: ParametricCurve>(axes: &Axes, curve: &C, t: f64, side: f64) -> Option<Degeneracy> {
    let (point, rate) = curve.eval_first(t);
    let local = axes.local(point);
    let across = (local.x * local.x + local.y * local.y).sqrt();
    let near = 16.0 * f64::EPSILON * (point.to_vec3().norm() + axes.origin.norm());
    if across > near || across.is_nan() {
        return None;
    }
    let (rate, up) = (rate.rate(), axes.normal.get());
    let square = rate.dot(up).abs() <= 16.0 * f64::EPSILON * rate.norm() && rate.norm() > 0.0;
    let normal = square.then_some(if side < 0.0 { axes.normal.reversed() } else { axes.normal });
    let point = Point::from_vec3(axes.origin + up * local.z);
    Some(Degeneracy { param: Param::V, at: t, point, normal })
}

/// How far from the axis's point at the target's height the foot's point of the curve can be, given the meridian
/// `(rho, h)` of the target and of a point of the curve: the target is `d` from that meridian point, so the foot is no
/// further from it, and turning keeps distance from the axis and height along it, so the foot's point is within
/// `rho + d` of the axis and `d` of the target's height.
fn meridian_reach(target: Vec3, meridian: Vec3) -> f64 {
    let gap = (meridian - target).norm();
    let reach = target.x + gap;
    (reach * reach + gap * gap).sqrt()
}

impl<C: ParametricCurve> Revolution<C> {
    /// `curve` turned about the axis through `origin` along `along`, `u` from the curve itself the way the axis turns.
    /// The curve may run without end, and the surface's `v` then does too.
    #[must_use]
    pub fn new(origin: Point<World>, along: Direction<World>, curve: C) -> Self {
        let range = curve.range();
        let axes = Axes::about(origin.to_vec3(), along);
        let ends = [(range.lo, -1.0), (range.hi, 1.0)];
        let poles = ends.into_iter().filter_map(|(t, side)| collapse(&axes, &curve, t, side)).collect();
        Self { axes, curve, range, poles }
    }

    /// `(cos u, sin u)` and the curve's point less the origin at `v`, from `point`.
    #[inline(always)]
    fn prepare<R: Real>(&self, u: R, point: Point<World, R>) -> ((R, R), Vec3<R>) {
        let (sin, cos) = sin_cos(u);
        ((cos, sin), point.to_vec3() - splat(self.axes.origin))
    }

    /// The meridian at `v`, its point and first two derivatives: the curve's distance `ρ` from the axis and its height
    /// `h` along it, as `(ρ, h, 0)`, with `ρ' = (a a' + b b') / ρ` and `ρ'' = (a'² + b'² + a a'' + b b'' − ρ'²) / ρ`
    /// for the curve's coordinates `a` and `b` across the axis. Where the curve is on the axis they are not numbers.
    fn meridian(&self, v: f64) -> [Vec3; 3] {
        let (point, first, second) = self.curve.eval_second(v);
        let (across, aside, up) = (self.axes.x, self.axes.y, self.axes.normal.get());
        let frame = |rate: Vec3| Vec3::new(rate.dot(across), rate.dot(aside), rate.dot(up));
        let (w, d1, d2) = (self.axes.local(point), frame(first.rate()), frame(second.rate()));
        let rho = (w.x * w.x + w.y * w.y).sqrt();
        let rate = (w.x * d1.x + w.y * d1.y) / rho;
        let change = (d1.x * d1.x + d1.y * d1.y + w.x * d2.x + w.y * d2.y - rate * rate) / rho;
        [Vec3::new(rho, w.z, 0.0), Vec3::new(rate, d1.z, 0.0), Vec3::new(change, d2.z, 0.0)]
    }

    /// The point, given what [`Revolution::prepare`] gives.
    #[inline(always)]
    fn located<R: Real>(&self, turn: (R, R), w: Vec3<R>) -> Point<World, R> {
        Point::from_vec3(splat(self.axes.origin) + partial(&self.axes, turn, w, 0))
    }
}

impl<C: ParametricCurve> Parametric for Revolution<C> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        let (turn, w) = self.prepare(uv.u(), self.curve.eval_point(uv.v()));
        self.located(turn, w)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let (point, rate) = self.curve.eval_first(uv.v());
        let (turn, w) = self.prepare(uv.u(), point);
        let du = partial(&self.axes, turn, w, 1);
        Jet1 {
            point: self.located(turn, w),
            du: Tangent::new(du),
            dv: Tangent::new(partial(&self.axes, turn, rate.rate(), 0)),
        }
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let (point, rate, change) = self.curve.eval_second(uv.v());
        let (turn, w) = self.prepare(uv.u(), point);
        let at = |w: Vec3<R>, i: u8| Tangent::new(partial(&self.axes, turn, w, i));
        let first = Jet1 { point: self.located(turn, w), du: at(w, 1), dv: at(rate.rate(), 0) };
        Jet2 { first, duu: at(w, 2), duv: at(rate.rate(), 1), dvv: at(change.rate(), 0) }
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let mut rates = Vec::with_capacity(usize::from(order));
        self.curve.derivatives(uv.v(), order, &mut rates, scratch)?;
        let (turn, w) = self.prepare(uv.u(), self.curve.eval_point(uv.v()));
        let none = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        for degree in 1..=order {
            for (i, j) in (0..=degree).rev().zip(0..=degree) {
                let rate = usize::from(j).checked_sub(1).map_or(Some(w), |k| rates.get(k).map(|t| t.rate()));
                out.push(Tangent::new(partial(&self.axes, turn, rate.unwrap_or(none), i)));
            }
        }
        Ok(())
    }

    fn domain(&self) -> Domain {
        Domain { u: turn(), v: self.range }
    }

    /// The solid cylinder about the axis that the box of the curve's part turns within: as far from the axis as the
    /// box's furthest corner, and between its lowest and highest along it, padded for rounding.
    fn bounds(&self, patch: UvBox) -> Bounds {
        let (v0, v1) = ordered(patch.lo.v(), patch.hi.v());
        if !finite(patch) {
            return everything();
        }
        let local = corners(&self.curve.bounds(v0, v1)).map(|c| self.axes.local(Point::from_vec3(c)));
        let reach = local.iter().map(|w| (w.x * w.x + w.y * w.y).sqrt()).fold(0.0, |a, x| if x > a { x } else { a });
        let (low, high) = local.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), w| {
            (if w.z < lo { w.z } else { lo }, if w.z > hi { w.z } else { hi })
        });
        let (o, x, y, z) = (self.axes.origin, self.axes.x, self.axes.y, self.axes.normal.get());
        let extent = |o: f64, x: f64, y: f64, z: f64| {
            let (along, spread) = (ordered(z * low, z * high), reach * (x * x + y * y).sqrt());
            (o + along.0 - spread, o + along.1 + spread)
        };
        let [(lx, hx), (ly, hy), (lz, hz)] =
            [extent(o.x, x.x, y.x, z.x), extent(o.y, x.y, y.y, z.y), extent(o.z, x.z, y.z, z.z)];
        let size = o.x.abs() + o.y.abs() + o.z.abs() + reach + low.abs() + high.abs();
        let margin = Length::from_millimetres(32.0 * f64::EPSILON * size);
        let corner = |x: f64, y: f64, z: f64| Point::<World>::from_vec3(Vec3::new(x, y, z));
        let held = Bounds::around(corner(lx, ly, lz), margin).union(&Bounds::around(corner(hx, hy, hz), margin));
        if size.is_finite() { held } else { everything() }
    }

    /// Exactly the foot over the whole turn: at each `v` the nearest `u` turns the curve's point to `p`'s angle, where
    /// the squared distance is the meridian's from `p`'s distance from the axis and height along it. So `v` is the
    /// curves' shared search on the meridian, which finds every valley of one parameter, and `u` the difference of the
    /// two angles.
    ///
    /// Where the curve runs without end, the search is over a finite window. The meridian at the curve's own foot of
    /// `p` is some distance `d` from `p`'s, so the foot is no further, and turning about the axis keeps distance from it
    /// and height along it: the foot's point of the curve is within `ρ + d` of the axis and `d` of `p`'s height, so
    /// within `√((ρ + d)² + d²)` of the axis's point at that height, for `ρ` `p`'s distance from the axis.
    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let at = self.axes.local(p);
        let target = Vec3::new((at.x * at.x + at.y * at.y).sqrt(), at.z, 0.0);
        let range = window(&self.curve, || {
            let anchor = self.curve.invert(p, None, tol, scratch).unwrap_or_else(|_| self.range.clamp(0.0));
            let [meridian, ..] = self.meridian(anchor);
            let level = self.axes.origin + self.axes.normal.get() * at.z;
            (anchor, Point::from_vec3(level), None, meridian_reach(target, meridian))
        });
        let v = nearest_by(|t| self.meridian(t), target, range);
        let on = self.axes.local(self.curve.eval_point(v));
        Ok(UvPoint::new(above(atan2(at.y, at.x) - atan2(on.y, on.x), 0.0), v))
    }

    fn family(&self) -> FamilyName {
        FamilyName("revolution")
    }

    fn content(&self, out: &mut Content) {
        self.axes.push(out);
        push_curve(out, &self.curve);
    }

    fn shape(&self) -> Shape {
        Shape { form: "revolution", values: with_curve(self.axes.values("origin"), &self.curve) }
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        &self.poles
    }

    /// The curve's B-spline over the patch's `v`, turned through the circle's arcs over its `u`, where the curve has
    /// one.
    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        let (u, v) = exact::part(Parametric::domain(self), patch)?;
        let form = self.curve.to_bspline(v.lo, v.hi)?;
        exact::revolved(&self.axes, (u.lo, u.hi), exact::profile(&self.axes, form))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use occam_units::{Angle, Local, Placement};

    use super::*;
    use crate::conformance::{
        self, Check, arc, bow, branch, differ, endless, lifted_bow, local, oval, placement, ring, segment, twist,
        upright_oval, whole_bow, whole_branch,
    };
    use crate::surface::Surface;
    use crate::{Circle, Cone, Cylinder, Line, Sphere};

    fn origin() -> Point<World> {
        Point::from_vec3(local(0.0, 0.0, 0.0))
    }

    fn up() -> Direction<World> {
        placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)))
    }

    fn turned<C: ParametricCurve>(curve: C) -> Revolution<C> {
        Revolution::new(origin(), up(), curve)
    }

    /// The suite over the curve turned, across the seam at `u` = 0 and over the curve's range.
    fn passes<C: ParametricCurve>(curve: C) {
        let range = curve.range();
        passes_over(curve, (range.lo, range.hi));
    }

    /// The suite over the curve turned, across the seam at `u` = 0 and over its stretch from `lo` to `hi`.
    fn passes_over<C: ParametricCurve>(curve: C, (lo, hi): (f64, f64)) {
        let (family, s) = (curve.family(), turned(curve));
        for (from, to) in [(-1.0, 2.5), (0.5, 6.0)] {
            let window = UvBox::new(UvPoint::new(from, lo), UvPoint::new(to, hi));
            let failures = conformance::surface(&s, window);
            assert!(failures.is_empty(), "{family:?} over {window:?}: {failures:#?}");
        }
    }

    /// The foot of the point `off` from the surface at `uv` along its normal: within the band of that far, at `uv`.
    fn found_from_afar(s: &dyn Surface, uv: UvPoint, off: f64) {
        let jet = s.first(uv);
        let normal = jet.du.rate().cross(jet.dv.rate());
        let p = Point::from_vec3(jet.point.to_vec3() + normal * (off / normal.norm()));
        let foot = s.closest(p, None, conformance::tolerance(), &mut Scratch::default()).unwrap();
        let band = conformance::tolerance().band().millimetres();
        assert!((foot.distance.millimetres() - off).abs() <= band, "{uv:?}: {foot:?}");
        assert!((foot.point.to_vec3() - jet.point.to_vec3()).norm() <= 1e-6, "{uv:?}: {foot:?}");
    }

    #[test]
    fn the_foot_where_the_surface_passes_through_itself_is_found() {
        // The ellipse turned about the axis passes through itself: this point's surface lies 7e-4 from another sheet,
        // where a grid of samples over both parameters found its foot.
        let s = turned(oval());
        let p = s.point(UvPoint::new(0.598_333_333_333_333_6, -0.63));
        let foot = Surface::closest(&s, p, None, conformance::tolerance(), &mut Scratch::default()).unwrap();
        assert!(foot.distance.millimetres() < 1e-12, "{foot:?}");
    }

    #[test]
    fn a_revolution_of_every_curve_family_passes_the_suite() {
        passes(segment());
        passes(arc());
        passes(ring());
        passes(oval());
        passes(upright_oval());
        passes(branch());
        passes(bow());
        passes(twist());
    }

    #[test]
    fn the_suite_catches_a_revolution_with_a_wrong_second_derivative() {
        let window = UvBox::new(UvPoint::new(-1.0, 0.5), UvPoint::new(2.5, 4.0));
        let failed = conformance::checks(&conformance::surface(&conformance::wrong_second(turned(arc())), window));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
    }

    /// A line through `start` in [`placement`]'s frame, along `along` in it, from `lo` to `hi`.
    fn line(start: [f64; 3], along: [f64; 3], (lo, hi): (f64, f64)) -> Line {
        let along = placement().direction(Direction::normalised(Vec3::new(along[0], along[1], along[2])));
        Line::new(Point::from_vec3(local(start[0], start[1], start[2])), along).trimmed(lo, hi).unwrap()
    }

    #[test]
    fn a_revolved_line_is_a_cylinder_or_a_cone_within_the_band() {
        let window = UvBox::new(UvPoint::new(-1.0, -4.0), UvPoint::new(5.0, 6.0));
        let tube = turned(line([6.0, 0.0, 0.0], [0.0, 0.0, 1.0], (-4.0, 6.0)));
        let cylinder = Cylinder::placed(&placement(), Length::from_millimetres(6.0)).unwrap();
        assert_eq!(differ(&tube, &cylinder, window), None);
        let (sin, cos) = sin_cos(0.4);
        let flare = turned(line([3.0, 0.0, 0.0], [sin, 0.0, cos], (-4.0, 6.0)));
        let cone = Cone::placed(&placement(), Length::from_millimetres(3.0), Angle::from_radians(0.4)).unwrap();
        assert_eq!(differ(&flare, &cone, window), None);
        assert!(Surface::degeneracies(&flare).is_empty(), "the apex, below the line's range, is not on the surface");
    }

    #[test]
    fn a_revolved_half_circle_is_a_sphere_with_its_poles_declared() {
        // A circle in the plane through the axis and placement()'s X axis, from one pole to the other.
        let upright = Placement::<Local, Local>::rotation(
            Direction::normalised(Vec3::new(1.0, 0.0, 0.0)),
            Angle::from_radians(FRAC_PI_2),
        );
        let half = Circle::placed(&upright.then(&placement()), Length::from_millimetres(5.0)).unwrap();
        let ball = turned(half.trimmed(-FRAC_PI_2, FRAC_PI_2).unwrap());
        let sphere = Sphere::placed(&placement(), Length::from_millimetres(5.0)).unwrap();
        let window = UvBox::new(UvPoint::new(0.0, -FRAC_PI_2), UvPoint::new(6.0, FRAC_PI_2));
        let poles = Surface::degeneracies(&ball);
        assert_eq!(poles.len(), 2);
        for (pole, want) in poles.iter().zip(Surface::degeneracies(&sphere)) {
            assert!((pole.point.to_vec3() - want.point.to_vec3()).norm() < 1e-13, "{pole:?} against {want:?}");
            assert_eq!(pole.normal, want.normal);
        }
        let failures = conformance::surface(&ball, window);
        assert!(failures.is_empty(), "{failures:#?}");
        let inside = UvBox::new(UvPoint::new(-1.0, -1.2), UvPoint::new(PI, 1.4));
        assert_eq!(differ(&ball, &sphere, inside), None);
    }

    #[test]
    fn a_revolution_of_a_curve_without_end_passes_the_suite_over_any_stretch() {
        passes_over(endless(), (-6.0, 9.0));
        passes_over(endless(), (110.0, 140.0));
        // A parabola in a plane square to the axis turned sweeps a flat ring that lies over itself, where a foot is any
        // of two, facing either way: the tilted one does not.
        passes_over(lifted_bow(), (-3.0, 4.0));
        passes_over(lifted_bow(), (10.0, 14.0));
        passes_over(whole_branch(), (-1.5, 1.8));
        let v = Surface::domain(&turned(whole_branch())).v;
        assert_eq!((v.lo, v.hi), (f64::NEG_INFINITY, f64::INFINITY));
    }

    #[test]
    fn a_line_without_end_turned_is_the_cylinder_or_the_cone() {
        let window = UvBox::new(UvPoint::new(-1.0, -40.0), UvPoint::new(5.0, 60.0));
        let along = |x: f64, z: f64| placement().direction(Direction::normalised(Vec3::new(x, 0.0, z)));
        let tube = turned(Line::new(Point::from_vec3(local(6.0, 0.0, 0.0)), along(0.0, 1.0)));
        let cylinder = Cylinder::placed(&placement(), Length::from_millimetres(6.0)).unwrap();
        assert_eq!(differ(&tube, &cylinder, window), None);
        let (sin, cos) = sin_cos(0.4);
        let flare = turned(Line::new(Point::from_vec3(local(3.0, 0.0, 0.0)), along(sin, cos)));
        let cone = Cone::placed(&placement(), Length::from_millimetres(3.0), Angle::from_radians(0.4)).unwrap();
        assert_eq!(differ(&flare, &cone, window), None, "both nappes, through the apex");
        assert!(Surface::degeneracies(&flare).is_empty(), "the apex is a crossing, not an end, so is not declared");
    }

    #[test]
    fn a_foot_far_out_along_a_curve_without_end_is_found() {
        let tube = turned(Line::new(Point::from_vec3(local(6.0, 0.0, 0.0)), up()));
        let foot = Surface::closest(
            &tube,
            Point::from_vec3(local(0.0, 9.0, 5000.0)),
            None,
            conformance::tolerance(),
            &mut Scratch::default(),
        )
        .unwrap();
        assert!((foot.distance.millimetres() - 3.0).abs() < 1e-9 && (foot.at.v() - 5000.0).abs() < 1e-9, "{foot:?}");
        found_from_afar(&tube, UvPoint::new(2.0, -3.0e4), 0.4);
        found_from_afar(&turned(endless()), UvPoint::new(0.7, 250.0), 0.3);
        found_from_afar(&turned(whole_bow()), UvPoint::new(4.0, 40.0), 0.3);
        found_from_afar(&turned(whole_branch()), UvPoint::new(2.0, 6.0), 0.3);
        found_from_afar(&turned(whole_branch()), UvPoint::new(5.0, -7.0), 0.3);
    }

    #[test]
    fn an_unbounded_patch_has_all_of_space_for_its_box_and_no_bspline() {
        let s = turned(whole_bow());
        let unbounded = UvBox::new(UvPoint::new(0.0, -2.0), UvPoint::new(1.0, f64::INFINITY));
        assert!(Surface::bounds(&s, unbounded).contains(Point::from_vec3(Vec3::new(1e300, -1e300, 1e300))));
        assert_eq!(Surface::to_bspline(&s, unbounded), None);
        let finite = UvBox::new(UvPoint::new(0.0, -2.0), UvPoint::new(1.0, 3.0));
        assert!(Surface::to_bspline(&s, finite).is_some());
    }

    #[test]
    fn its_shape_and_content_name_the_axis_and_the_curve() {
        let s = turned(arc());
        let shape = Surface::shape(&s);
        let names: Vec<&str> = shape.values.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(shape.form, "revolution");
        assert_eq!(names[..4], ["origin", "normal", "x-axis", "curve-centre"]);
        let (mut content, mut curve) = (Content::default(), Content::default());
        Surface::content(&s, &mut content);
        crate::Curve::content(&arc(), &mut curve);
        assert!(content.words().ends_with(curve.words()), "the curve's name and numbers come last");
        assert_eq!(content.words().len(), 1 + 10 + 12 + curve.words().len());
        let other = turned(arc().trimmed(0.5, 3.0).unwrap());
        let mut differs = Content::default();
        Surface::content(&other, &mut differs);
        assert_ne!(content, differs);
        assert_eq!(Surface::family(&s), FamilyName("revolution"));
    }

    #[test]
    fn its_box_holds_the_turned_curve_and_a_patch_not_finite_is_all_of_space() {
        let s = turned(oval());
        let all = UvBox::new(UvPoint::new(0.0, -1.0), UvPoint::new(TAU, 2.0));
        assert!(conformance::slack(&s, all, 60) < 8.0, "the cylinder about a whole turn is near the surface");
        let unbounded = UvBox::new(UvPoint::new(0.0, -1.0), UvPoint::new(f64::INFINITY, 2.0));
        assert!(Surface::bounds(&s, unbounded).contains(Point::from_vec3(Vec3::new(1e300, -1e300, 0.0))));
    }

    /// The segment from `from` to `to`, its parameter the length from `from`.
    fn stick(from: [f64; 3], to: [f64; 3]) -> Line {
        let (a, b) = (Vec3::new(from[0], from[1], from[2]), Vec3::new(to[0], to[1], to[2]));
        Line::new(Point::from_vec3(a), Direction::normalised(b - a)).trimmed(0.0, (b - a).norm()).unwrap()
    }

    /// Turned about `axis` through the world's origin.
    fn about<C: ParametricCurve>(axis: Vec3, curve: C) -> Revolution<C> {
        Revolution::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(axis), curve)
    }

    fn upright<C: ParametricCurve>(curve: C) -> Revolution<C> {
        about(Vec3::new(0.0, 0.0, 1.0), curve)
    }

    #[test]
    fn an_end_collapses_only_where_both_its_coordinates_across_the_axis_are_nothing() {
        // Off the axis with one coordinate across it nothing, or with the two as large: no line collapses.
        for start in [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [1.0, 1.0, 0.0], [1.0, -1.0, 0.0]] {
            assert_eq!(Surface::degeneracies(&upright(stick(start, [3.0, 1.0, 4.0]))), &[][..], "{start:?}");
        }
        // At the origin, on an axis through it, the numbers there are nothing, and so is the rounding allowed.
        let at_origin = upright(stick([0.0, 0.0, 0.0], [2.0, 0.0, 1.0]));
        let [line] = Surface::degeneracies(&at_origin) else { panic!("{:?}", at_origin.poles) };
        assert_eq!((line.param, line.at, line.point.to_vec3()), (Param::V, 0.0, Vec3::new(0.0, 0.0, 0.0)));
        // On a tilted axis through the origin, an end on it is there only to rounding, which is allowed.
        let axis = Vec3::new(1.0, 2.0, 3.0) * (1.0 / 14f64.sqrt());
        for k in [0.7, 2.0, 3.3, 5.1] {
            let start = axis * k;
            let s = about(axis, stick([start.x, start.y, start.z], [4.0, -1.0, 2.0]));
            assert_eq!(Surface::degeneracies(&s).len(), 1, "{k}");
        }
    }

    #[test]
    fn an_end_on_the_axis_has_a_normal_only_where_the_curve_leaves_it_square() {
        // Leaving at 45 degrees: a cone's apex, with no normal.
        let oblique = upright(stick([0.0, 0.0, 1.0], [2.0, 0.0, 3.0]));
        let [apex] = Surface::degeneracies(&oblique) else { panic!() };
        assert_eq!(apex.normal, None);
        // Leaving square to a tilted axis at a rate of 1000, so the rounding of its part along the axis is large: the
        // normal points along the axis away from the curve's other end.
        let axis = Vec3::new(1.0, 2.0, 3.0) * (1.0 / 14f64.sqrt());
        let across = axis.cross(Vec3::new(0.3, -0.7, 0.2));
        let (start, square) = (axis * 2.0, across * (1000.0 / across.norm()));
        let poles = vec![Point::from_vec3(start), Point::from_vec3(start + square)];
        let s = about(axis, crate::SplineCurve::bezier(poles, None).unwrap());
        let [pole] = Surface::degeneracies(&s) else { panic!() };
        assert!(pole.normal.is_some_and(|n| (n.get() + axis).norm() < 1e-15), "{pole:?}");
        // A curve standing still at its end on the axis leaves it in no direction: no normal.
        let still = vec![
            Point::from_vec3(Vec3::new(0.0, 0.0, 1.0)),
            Point::from_vec3(Vec3::new(0.0, 0.0, 1.0)),
            Point::from_vec3(Vec3::new(2.0, 0.0, 1.0)),
        ];
        let s = upright(crate::SplineCurve::bezier(still, None).unwrap());
        let [stopped] = Surface::degeneracies(&s) else { panic!() };
        assert_eq!(stopped.normal, None);
    }

    #[test]
    fn the_meridian_s_rates_are_the_differences_of_its_distance_and_height() {
        let s = turned(oval());
        let h = 1e-5;
        for v in [-0.7, 0.1, 0.9, 1.6] {
            let [_, rate, change] = s.meridian(v);
            let ([up, up_rate, _], [down, down_rate, _]) = (s.meridian(v + h), s.meridian(v - h));
            let pairs = [
                (rate.x, (up.x - down.x) / (2.0 * h)),
                (rate.y, (up.y - down.y) / (2.0 * h)),
                (change.x, (up_rate.x - down_rate.x) / (2.0 * h)),
                (change.y, (up_rate.y - down_rate.y) / (2.0 * h)),
            ];
            for (k, (got, want)) in pairs.into_iter().enumerate() {
                assert!((got - want).abs() <= 1e-6 * (1.0 + want.abs()), "{v} {k}: {got} against {want}");
            }
        }
    }

    #[test]
    fn the_foot_is_sought_within_the_target_s_distance_from_the_axis_and_a_known_gap() {
        // The target at 3 from the axis and 1 up, a point of the curve's meridian at (5, 4): 13 apart squared, so the
        // foot's point is within 3 + the root of 13 of the axis and the root of 13 of the height.
        let gap = 13f64.sqrt();
        let want = ((3.0 + gap) * (3.0 + gap) + 13.0).sqrt();
        assert_eq!(meridian_reach(Vec3::new(3.0, 1.0, 0.0), Vec3::new(5.0, 4.0, 0.0)), want);
    }

    #[test]
    fn a_box_is_padded_by_thirty_two_epsilons_of_its_size() {
        // About Z through (1, -2, 0.5), a segment from (2, -2, 0) to (3, -2, 3): 1 to 2 from the axis, from 0.5 below
        // the origin to 2.5 above, so the box's lowest z is 0, where the margin can be read, and the size is the
        // origin's 1 + 2 + 0.5, the reach of 2, and the heights' 0.5 + 2.5.
        let origin = Point::from_vec3(Vec3::new(1.0, -2.0, 0.5));
        let s = Revolution::new(
            origin,
            Direction::normalised(Vec3::new(0.0, 0.0, 1.0)),
            stick([2.0, -2.0, 0.0], [3.0, -2.0, 3.0]),
        );
        let b = Surface::bounds(&s, UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(TAU, 10f64.sqrt())));
        let margin = 32.0 * f64::EPSILON * 8.5;
        let got = -b.low().to_vec3().z;
        assert!((got - margin).abs() <= 1e-2 * margin, "{got} against {margin}: {b:?}");
    }
}
