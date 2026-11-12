//! Source: original
//! The surface of linear extrusion: a curve swept along a direction (PARITY 2.2), `C(u) + v d`, with `u` the curve's
//! parameter and `v` a length along the unit direction `d`. Its partial derivatives in `u` are the curve's, in `v` once
//! `d`, and every mixed one is zero. The curve is held by type, so its rungs run in lanes, and may run without end,
//! though not along `d`, which makes no surface. Its foot is the curve's foot seen along `d`, then the length along `d`
//! to the point.

use occam_math::Real;
use occam_report::{Code, Fault, Report};
use occam_units::{Bounds, Direction, Length, ParamKind, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::axes::direction;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::BsplineSurface;
use crate::curve::ParametricCurve;
use crate::domain::{Domain, ParamRange, UvBox};
use crate::exact;
use crate::identity::{Content, FamilyName, Shape};
use crate::nearest::nearest_by;
use crate::support::{everything, splat};
use crate::surface::Parametric;
use crate::swept::{corners, finite, ordered, push_curve, window, with_curve};

/// A curve swept along a direction.
#[derive(Clone, Debug, PartialEq)]
pub struct Extrusion<C> {
    curve: C,
    /// The direction, and it as a unit vector.
    direction: Direction<World>,
    along: Vec3,
    range: ParamRange,
}

impl<C: ParametricCurve> Extrusion<C> {
    /// `curve` swept along `along`, `v` a length from the curve itself. The curve may run without end, and the
    /// surface's `u` then does too.
    ///
    /// # Errors
    ///
    /// `geom.swept-along-itself` for a curve no finite stretch of which holds its points within a distance of a line
    /// along `along`: a line along `along`, which swept makes no surface, and a hyperbola along an asymptote without
    /// end, which makes a half-plane whose edge is at infinity, where a point beside it has no foot and the curve's
    /// points are rounding long before the search could approach it.
    pub fn new(curve: C, along: Direction<World>) -> Result<Self, Report> {
        let start = curve.eval_point(curve.range().clamp(0.0));
        if curve.within(start, Some(along), 1.0).is_none() {
            return Err(Report::new(Code::SweptAlongItself, Fault::InvalidInput)
                .with_value(direction("direction", along.get())));
        }
        Ok(Self { range: curve.range(), curve, direction: along, along: along.get() })
    }

    /// `point` moved `v` along the direction.
    #[inline(always)]
    fn moved<R: Real>(&self, point: Point<World, R>, v: R) -> Point<World, R> {
        Point::from_vec3(point.to_vec3() + splat(self.along) * v)
    }

    /// The curve at `t` seen along the direction: its point and first two derivatives, each with its part along the
    /// direction taken away.
    fn across(&self, t: f64) -> [Vec3; 3] {
        let (point, first, second) = self.curve.eval_second(t);
        [point.to_vec3(), first.rate(), second.rate()].map(|v| v - self.along * v.dot(self.along))
    }
}

impl<C: ParametricCurve> Parametric for Extrusion<C> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.moved(self.curve.eval_point(uv.u()), uv.v())
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let (point, rate) = self.curve.eval_first(uv.u());
        Jet1 { point: self.moved(point, uv.v()), du: rate, dv: Tangent::new(splat(self.along)) }
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let (point, rate, change) = self.curve.eval_second(uv.u());
        let first = Jet1 { point: self.moved(point, uv.v()), du: rate, dv: Tangent::new(splat(self.along)) };
        let zero = Tangent::new(splat(Vec3::new(0.0, 0.0, 0.0)));
        Jet2 { first, duu: change, duv: zero, dvv: zero }
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let mut rates = Vec::with_capacity(usize::from(order));
        self.curve.derivatives(uv.u(), order, &mut rates, scratch)?;
        let zero = Tangent::new(Vec3::new(0.0, 0.0, 0.0));
        for degree in 1..=order {
            for (i, j) in (0..=degree).rev().zip(0..=degree) {
                out.push(match (i, j) {
                    (0, 1) => Tangent::new(self.along),
                    (_, 0) => rates.get(usize::from(i).saturating_sub(1)).copied().unwrap_or(zero),
                    _ => zero,
                });
            }
        }
        Ok(())
    }

    fn domain(&self) -> Domain {
        Domain { u: self.range, v: ParamRange::unbounded(ParamKind::Length) }
    }

    /// The box of the curve's part, swept from `v` at one end of the patch to the other, padded for rounding.
    fn bounds(&self, patch: UvBox) -> Bounds {
        let ((u0, u1), (v0, v1)) = (ordered(patch.lo.u(), patch.hi.u()), ordered(patch.lo.v(), patch.hi.v()));
        if !finite(patch) {
            return everything();
        }
        let hull = self.curve.bounds(u0, u1);
        let swept = corners(&hull).into_iter().flat_map(|c| [c + self.along * v0, c + self.along * v1]);
        let Some(held) = Bounds::of(swept.map(Point::<World>::from_vec3)) else { return everything() };
        let (low, high) = (held.low().to_vec3(), held.high().to_vec3());
        let size = [low.x, low.y, low.z, high.x, high.y, high.z].into_iter().map(f64::abs).fold(0.0, |a, x| a + x);
        let margin = Length::from_millimetres(16.0 * f64::EPSILON * size);
        Bounds::around(held.low(), margin).union(&Bounds::around(held.high(), margin))
    }

    /// Exactly the foot: at each `u` the nearest `v` is `(p - C(u)) . d`, where the squared distance is the curve's,
    /// seen along `d`, from `p` seen the same way. So `u` is the curves' shared search on the curve with its part along
    /// `d` taken away, which finds every valley of one parameter, and `v` follows.
    ///
    /// Where the curve runs without end, the search is over a finite window: the curve's own foot of `p`, seen along
    /// `d`, is some distance from `p` seen the same way, so the foot is no further, and its `u` is among those the
    /// curve says come within that distance of the line through `p` along `d`.
    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let q = p.to_vec3();
        let seen = q - self.along * q.dot(self.along);
        let range = window(&self.curve, || {
            let anchor = self.curve.invert(p, None, tol, scratch).unwrap_or_else(|_| self.range.clamp(0.0));
            let [point, ..] = self.across(anchor);
            (anchor, p, Some(self.direction), (point - seen).norm())
        });
        let u = nearest_by(|t| self.across(t), seen, range);
        Ok(UvPoint::new(u, (q - self.curve.eval_point(u).to_vec3()).dot(self.along)))
    }

    fn family(&self) -> FamilyName {
        FamilyName("extrusion")
    }

    fn content(&self, out: &mut Content) {
        out.push_vec3(self.along);
        push_curve(out, &self.curve);
    }

    fn shape(&self) -> Shape {
        Shape { form: "extrusion", values: with_curve(vec![direction("direction", self.along)], &self.curve) }
    }

    /// The curve's B-spline over the patch's `u`, moved to each end of its `v`, where the curve has one.
    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        let (u, v) = exact::part(Parametric::domain(self), patch)?;
        Some(exact::extruded(self.curve.to_bspline(u.lo, u.hi)?, self.along, (v.lo, v.hi)))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use super::*;
    use crate::conformance::{
        self, Check, arc, bow, branch, differ, endless, local, oval, placement, ring, segment, twist, whole_bow,
        whole_branch,
    };
    use crate::surface::Surface;
    use crate::{Circle, Cylinder, Line, Plane};

    /// A direction across every curve the tests sweep, at a slant to [`placement`]'s Z axis.
    fn slant() -> Direction<World> {
        placement().direction(Direction::normalised(Vec3::new(0.3, -0.2, 1.0)))
    }

    fn swept<C: ParametricCurve>(curve: C) -> Extrusion<C> {
        Extrusion::new(curve, slant()).unwrap()
    }

    /// The suite over the curve swept, over its range and a stretch either side of it.
    fn passes<C: ParametricCurve>(curve: C) {
        let range = curve.range();
        passes_over(curve, (range.lo, range.hi), slant());
    }

    /// The suite over the curve swept along `along`, over its stretch from `lo` to `hi` and a stretch either side of it.
    fn passes_over<C: ParametricCurve>(curve: C, (lo, hi): (f64, f64), along: Direction<World>) {
        let (family, s) = (curve.family(), Extrusion::new(curve, along).unwrap());
        for (from, to) in [(-3.0, 4.0), (0.5, 1.5)] {
            let window = UvBox::new(UvPoint::new(lo, from), UvPoint::new(hi, to));
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

    /// A direction along the first asymptote of [`whole_branch`], `a x + b y` in its frame.
    fn asymptote() -> Direction<World> {
        let along = whole_branch().eval_first(30.0).1.rate();
        Direction::normalised(along)
    }

    #[test]
    fn an_extrusion_of_a_curve_without_end_passes_the_suite_over_any_stretch() {
        passes_over(endless(), (-6.0, 9.0), slant());
        passes_over(endless(), (110.0, 140.0), slant());
        passes_over(whole_bow(), (-3.0, 4.0), slant());
        passes_over(whole_branch(), (-1.5, 1.8), slant());
        passes_over(branch(), (-1.2, 1.0), asymptote());
        let u = Surface::domain(&swept(whole_bow())).u;
        assert_eq!((u.lo, u.hi), (f64::NEG_INFINITY, f64::INFINITY));
    }

    #[test]
    fn a_line_without_end_swept_is_the_plane() {
        let x = placement().direction(Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        let y = placement().direction(Direction::normalised(Vec3::new(0.0, 1.0, 0.0)));
        let sheet = Extrusion::new(Line::new(Point::from_vec3(local(0.0, 0.0, 0.0)), x), y).unwrap();
        let window = UvBox::new(UvPoint::new(-40.0, -30.0), UvPoint::new(60.0, 25.0));
        assert_eq!(differ(&sheet, &Plane::placed(&placement()), window), None);
        let p = Point::from_vec3(local(1.0e4, -3.0e3, 0.7));
        let foot = Surface::closest(&sheet, p, None, conformance::tolerance(), &mut Scratch::default()).unwrap();
        assert!((foot.distance.millimetres() - 0.7).abs() < 1e-9, "{foot:?}");
        assert!((foot.at.u() - 1.0e4).abs() < 1e-8 && (foot.at.v() + 3.0e3).abs() < 1e-8, "{foot:?}");
    }

    #[test]
    fn a_foot_far_out_along_a_curve_without_end_is_found() {
        found_from_afar(&swept(endless()), UvPoint::new(250.0, -7.0), 0.3);
        found_from_afar(&swept(whole_bow()), UvPoint::new(40.0, 3.0), 0.3);
        found_from_afar(&swept(whole_bow()), UvPoint::new(-35.0, -2.0), 0.3);
        found_from_afar(&swept(whole_branch()), UvPoint::new(6.0, 1.0), 0.3);
        found_from_afar(&swept(whole_branch()), UvPoint::new(-5.0, 2.0), 0.3);
    }

    #[test]
    fn an_unbounded_patch_has_all_of_space_for_its_box_and_no_bspline() {
        let s = swept(whole_bow());
        let unbounded = UvBox::new(UvPoint::new(f64::NEG_INFINITY, 0.0), UvPoint::new(1.0, 2.0));
        assert!(Surface::bounds(&s, unbounded).contains(Point::from_vec3(Vec3::new(1e300, -1e300, 1e300))));
        assert_eq!(Surface::to_bspline(&s, unbounded), None);
        assert!(Surface::to_bspline(&s, UvBox::new(UvPoint::new(-1.0, 0.0), UvPoint::new(1.0, 2.0))).is_some());
    }

    #[test]
    fn an_extrusion_of_every_curve_family_passes_the_suite() {
        passes(segment());
        passes(arc());
        passes(ring());
        passes(oval());
        passes(branch());
        passes(bow());
        passes(twist());
    }

    #[test]
    fn the_suite_catches_an_extrusion_with_a_wrong_second_derivative() {
        let window = UvBox::new(UvPoint::new(0.5, -3.0), UvPoint::new(4.0, 4.0));
        let failed = conformance::checks(&conformance::surface(&conformance::wrong_second(swept(arc())), window));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
    }

    #[test]
    fn an_extruded_circle_is_a_cylinder_within_the_band() {
        let up = placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
        let circle = Circle::placed(&placement(), Length::from_millimetres(6.0)).unwrap();
        let tube = Extrusion::new(circle, up).unwrap();
        let cylinder = Cylinder::placed(&placement(), Length::from_millimetres(6.0)).unwrap();
        let window = UvBox::new(UvPoint::new(0.0, -15.0), UvPoint::new(TAU, 25.0));
        assert_eq!(differ(&tube, &cylinder, window), None);
    }

    #[test]
    fn a_line_swept_along_itself_is_refused_with_or_without_end() {
        let along = Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), slant());
        for line in [along, along.trimmed(-1.0, 2.0).unwrap()] {
            let refused = Extrusion::new(line, slant()).unwrap_err();
            assert_eq!(
                (refused.code, refused.fault),
                (Code::SweptAlongItself, Some(occam_report::Fault::InvalidInput))
            );
            let named = refused.value("direction").map(|v| v.value);
            let d = slant().get();
            assert_eq!(named, Some(occam_report::Value::Point([d.x, d.y, d.z])));
        }
        let backwards = Direction::normalised(slant().get() * -1.0);
        assert_eq!(Extrusion::new(along, backwards).unwrap_err().code, Code::SweptAlongItself);
    }

    #[test]
    fn a_hyperbola_along_an_asymptote_is_refused_only_where_it_runs_that_way_without_end() {
        let refused = Extrusion::new(whole_branch(), asymptote()).unwrap_err();
        assert_eq!(refused.code, Code::SweptAlongItself);
        assert!(Extrusion::new(branch(), asymptote()).is_ok(), "trimmed, it is a finite strip of a half-plane");
        assert!(Extrusion::new(whole_branch().trimmed(-200.0, 3.0).unwrap(), asymptote()).is_ok(), "the other end");
        let other = Direction::normalised(whole_branch().eval_first(-30.0).1.rate() * -1.0);
        assert_eq!(Extrusion::new(whole_branch(), other).unwrap_err().code, Code::SweptAlongItself, "the other one");
        assert!(Extrusion::new(whole_branch(), slant()).is_ok());
    }

    #[test]
    fn its_shape_and_content_name_the_direction_and_the_curve() {
        let s = swept(bow());
        let shape = Surface::shape(&s);
        let names: Vec<&str> = shape.values.iter().map(|v| v.name.as_str()).collect();
        assert_eq!((shape.form, names[..2].to_vec()), ("extrusion", vec!["direction", "curve-vertex"]));
        let (mut content, mut curve) = (Content::default(), Content::default());
        Surface::content(&s, &mut content);
        crate::Curve::content(&bow(), &mut curve);
        assert!(content.words().ends_with(curve.words()), "the curve's name and numbers come last");
        assert_eq!(content.words().len(), 1 + 9 + 3 + curve.words().len());
        assert_eq!((Surface::family(&s), Surface::degeneracies(&s)), (FamilyName("extrusion"), &[][..]));
        let v = Surface::domain(&s).v;
        assert_eq!((v.lo, v.hi, v.kind), (f64::NEG_INFINITY, f64::INFINITY, ParamKind::Length));
    }

    #[test]
    fn its_box_holds_the_swept_curve_and_a_patch_not_finite_is_all_of_space() {
        let s = swept(oval());
        let patch = UvBox::new(UvPoint::new(-1.0, -2.0), UvPoint::new(2.0, 3.0));
        assert!(conformance::slack(&s, patch, 60) < 3.0, "the curve's box swept is near the surface");
        let unbounded = UvBox::new(UvPoint::new(-1.0, 0.0), UvPoint::new(2.0, f64::INFINITY));
        assert!(Surface::bounds(&s, unbounded).contains(Point::from_vec3(Vec3::new(1e300, -1e300, 0.0))));
    }

    #[test]
    fn a_box_is_padded_by_sixteen_epsilons_of_its_size() {
        // A segment along X from 0 to 2, swept up Z from 0 to 3: its box's lowest x and z are 0, where the margin can be
        // read to a part in a thousand, and its size, the sum of its corners' magnitudes, is 2 + 3.
        let along_x = Direction::normalised(Vec3::new(1.0, 0.0, 0.0));
        let segment = Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), along_x).trimmed(0.0, 2.0).unwrap();
        let s = Extrusion::new(segment, Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).unwrap();
        let b = Surface::bounds(&s, UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(2.0, 3.0)));
        let margin = 16.0 * f64::EPSILON * 5.0;
        for got in [-b.low().to_vec3().x, -b.low().to_vec3().z] {
            assert!((got - margin).abs() <= 1e-3 * margin, "{got} against {margin}: {b:?}");
        }
    }

    #[test]
    fn a_foot_far_along_the_direction_keeps_both_its_parameters() {
        // A circle of radius 1 lifted 0.75 up Z, swept up Z, and a point 1e9 up at angle 2 and radius 3: the foot is at
        // angle 2, the nearest of the circle, and 1e9 less 0.75 along. Far up, a distance taken with the height left in
        // could not tell the circle's nearest point from its ends.
        let lifted = occam_units::Placement::<World, World>::translation(occam_units::Vector::from_vec3(Vec3::new(
            0.0, 0.0, 0.75,
        )));
        let ring = Circle::placed(
            &occam_units::Placement::<occam_units::Local, World>::identity().then(&lifted),
            Length::from_millimetres(1.0),
        )
        .unwrap();
        let s = Extrusion::new(ring, Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).unwrap();
        let (sin, cos) = occam_math::sin_cos(2.0);
        let p = Point::from_vec3(Vec3::new(3.0 * cos, 3.0 * sin, 1e9));
        let uv = Surface::invert(&s, p, None, conformance::tolerance(), &mut Scratch::default()).unwrap();
        assert!((uv.u() - 2.0).abs() < 1e-9 && (uv.v() - (1e9 - 0.75)).abs() < 1e-6, "{uv:?}");
    }

    #[test]
    fn the_foot_on_a_slanting_sweep_of_a_curve_without_end_is_the_nearest() {
        // A parabola with its vertex at (10, 0, 0), swept along a slant: the curve's own foot of a point is not the
        // sweep's, so the search must reach as far from the point as the sweep through the curve's foot. Points across the
        // origin from the vertex, among others, have their own foot near it.
        let vertex = occam_units::Placement::<World, World>::translation(occam_units::Vector::from_vec3(Vec3::new(
            10.0, 0.0, 0.0,
        )));
        let placed = occam_units::Placement::<occam_units::Local, World>::identity().then(&vertex);
        let parabola = crate::Parabola::placed(&placed, Length::from_millimetres(1.0)).unwrap();
        let slant = Direction::normalised(Vec3::new(0.3, 0.5, 1.0));
        let s = Extrusion::new(parabola, slant).unwrap();
        let at = |q: Vec3, u: f64| {
            let c = ParametricCurve::eval_point(&parabola, u).to_vec3();
            (q - (c + slant.get() * (q - c).dot(slant.get()))).norm()
        };
        let points = [(-10.0, 0.0, 0.0), (-10.0, 0.5, -0.3), (-9.0, -1.0, 0.4), (-11.0, 1.0, 1.0), (4.0, 6.0, 1.5)];
        for (x, y, z) in points {
            let q = Vec3::new(x, y, z);
            let uv = Surface::invert(&s, Point::from_vec3(q), None, conformance::tolerance(), &mut Scratch::default());
            let got = (Surface::point(&s, uv.unwrap()).to_vec3() - q).norm();
            let best = (-40_000..=40_000_i32)
                .map(|k| at(q, f64::from(k) * 1e-3))
                .fold(f64::INFINITY, |m: f64, d| if d < m { d } else { m });
            assert!(got <= best + 1e-9, "({x}, {y}, {z}): {got} against {best}");
        }
    }
}
