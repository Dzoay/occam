//! Source: do-carmo-1976
//! The offset curve, `C + d N` with `N = (C' x V) / |C' x V|` for a reference direction `V` (PARITY 2.1): in a plane
//! square to `V`, the curve moved within its plane, to its right as it runs where `d` is above zero, as the systems
//! `FreeCAD` uses offset a curve. Its derivatives to an order need the base's to the order above, through the normal's,
//! which normal.rs gives exactly. Where `C'` runs along `V` the offset is undefined: NaN there. The base is held by
//! type, so the first two rungs run in lanes; the second needs the base's third derivative and runs a lane at a time.

use occam_math::Real;
use occam_report::{NamedValue, Report};
use occam_units::{Bounds, Direction, Length, Point, Tangent, Vec3, World};

use crate::axes::direction;
use crate::batch::Scratch;
use crate::bspline::BsplineCurve;
use crate::curve::ParametricCurve;
use crate::domain::ParamRange;
use crate::identity::{Content, FamilyName, Shape};
use crate::lanewise::{MAX_LANES, from_lanes, lanes};
use crate::normal;
use crate::support::{everything, finite_size, padded};
use crate::swept::{push_curve, with_curve};

/// A curve moved square to itself and to a reference direction.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetCurve<C> {
    base: C,
    distance: f64,
    /// The reference direction, of length one.
    reference: Vec3,
}

/// The offset's point and derivatives to `order` into `out`, from the base's to `order + 1` in `base`: `C + d N`.
/// `m` and `lengths` are workspace as long as `out`.
#[inline(always)]
fn offset<R: Real>(
    order: usize,
    (base, reference, distance): (&[Vec3<R>], Vec3, f64),
    (m, lengths): (&mut [Vec3<R>], &mut [R]),
    out: &mut [Vec3<R>],
) {
    normal::curve(order, base, reference, m, lengths);
    for (slot, (c, n)) in out.iter_mut().zip(base.iter().zip(m.iter())).take(order.saturating_add(1)) {
        *slot = *c + *n * R::splat(distance);
    }
}

impl<C: ParametricCurve> OffsetCurve<C> {
    /// `base` moved `distance` along `C' x reference` over its length.
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a distance that is not finite.
    pub fn new(base: C, distance: Length, reference: Direction<World>) -> Result<Self, Report> {
        Ok(Self { base, distance: finite_size(distance)?, reference: reference.get() })
    }

    /// The point and its derivatives to `order` at `t` as numbers, from the base's `derivatives` in the caller's
    /// `rates` and `scratch`, so that a call's lanes share one workspace.
    fn jets(&self, t: f64, order: u8, (rates, scratch): (&mut Vec<Tangent<World>>, &mut Scratch)) -> [Vec3; 3] {
        rates.clear();
        let nan = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        let mut base = [nan; 4];
        if ParametricCurve::derivatives(&self.base, t, order.saturating_add(1), rates, scratch).is_ok() {
            let point = std::iter::once(self.base.eval_point(t).to_vec3());
            for (slot, value) in base.iter_mut().zip(point.chain(rates.iter().map(|r| r.rate()))) {
                *slot = value;
            }
        }
        let (mut m, mut lengths, mut out) = ([nan; 3], [f64::NAN; 3], [nan; 3]);
        offset(usize::from(order), (&base, self.reference, self.distance), (&mut m, &mut lengths), &mut out);
        out
    }

    /// The offset's normal of a straight base from `from` to `to`, which is the same all along it.
    fn across(&self, from: Vec3, to: Vec3) -> Vec3 {
        let n = (to - from).cross(self.reference);
        n * (1.0 / n.norm())
    }
}

impl<C: ParametricCurve> ParametricCurve for OffsetCurve<C> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        let (point, rate) = self.base.eval_first(t);
        let zero = R::splat(0.0);
        let none = Vec3::new(zero, zero, zero);
        let (mut m, mut lengths, mut out) = ([none], [zero], [none]);
        let base = [point.to_vec3(), rate.rate()];
        offset(0, (&base, self.reference, self.distance), (&mut m, &mut lengths), &mut out);
        let [at] = out;
        Point::from_vec3(at)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        let (point, rate, change) = self.base.eval_second(t);
        let zero = R::splat(0.0);
        let none = Vec3::new(zero, zero, zero);
        let (mut m, mut lengths, mut out) = ([none; 2], [zero; 2], [none; 2]);
        let base = [point.to_vec3(), rate.rate(), change.rate()];
        offset(1, (&base, self.reference, self.distance), (&mut m, &mut lengths), &mut out);
        let [at, along] = out;
        (Point::from_vec3(at), Tangent::new(along))
    }

    /// A lane at a time, in one workspace: the second derivative needs the base's third, which no rung gives.
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        const { assert!(R::LANES <= MAX_LANES, "more lanes than a lane-at-a-time evaluation holds") };
        let ts = lanes(t);
        let nan = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        let (mut rates, mut scratch) = (Vec::with_capacity(3), Scratch::default());
        let each: [[Vec3; 3]; MAX_LANES] = std::array::from_fn(|k| match ts.get(k) {
            Some(&t) if k < R::LANES => self.jets(t, 2, (&mut rates, &mut scratch)),
            _ => [nan; 3],
        });
        let at = |slot: usize| from_lanes::<R>(|k| each.get(k).and_then(|e| e.get(slot)).copied().unwrap_or(nan));
        (Point::from_vec3(at(0)), Tangent::new(at(1)), Tangent::new(at(2)))
    }

    /// From the base's derivatives to the order above, through the caller's `out` and `scratch`.
    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let above = normal::above(order)?;
        let start = out.len();
        ParametricCurve::derivatives(&self.base, t, above, out, scratch)?;
        let own = usize::from(above);
        let (lengths, vectors) = scratch.reals_and_vectors(own, own.saturating_mul(3).saturating_add(1));
        let (base, rest) = vectors.split_at_mut(own.saturating_add(1));
        let (m, results) = rest.split_at_mut(own);
        normal::take(self.base.eval_point(t).to_vec3(), (out, start), base);
        offset(usize::from(order), (base, self.reference, self.distance), (m, lengths), results);
        out.extend(results.iter().skip(1).map(|v| Tangent::new(*v)));
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.base.range()
    }

    /// The base's box over the part, grown by the distance and padded for the rounding in the normal.
    /// The base's box grown by the distance only square to the reference: the offset moves each point by the distance
    /// along a unit vector square to the reference `V`, whose part along an axis is at most `sqrt(1 - V_i^2)` (Bessel's
    /// inequality, as the two are orthonormal). So a curve offset in its own plane keeps to it.
    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let b = self.base.bounds(lo, hi);
        let across = |v: f64| self.distance.abs() * (1.0 - v * v).abs().sqrt();
        let grown = Vec3::new(across(self.reference.x), across(self.reference.y), across(self.reference.z));
        let corners = [b.low().to_vec3() - grown, b.high().to_vec3() + grown].map(Point::from_vec3);
        Bounds::of(corners).map_or_else(everything, |hull| padded(hull, 8.0))
    }

    /// Every point of the offset is within the distance of the base's at the same parameter, so the base's window for
    /// a radius larger by the distance holds the offset's.
    fn within(&self, centre: Point<World>, along: Option<Direction<World>>, radius: f64) -> Option<(f64, f64)> {
        self.base.within(centre, along, radius + self.distance.abs())
    }

    fn family(&self) -> FamilyName {
        FamilyName("offset")
    }

    fn content(&self, out: &mut Content) {
        out.push(self.distance);
        out.push_vec3(self.reference);
        push_curve(out, &self.base);
    }

    fn shape(&self) -> Shape {
        let values = vec![NamedValue::length("distance", self.distance), direction("reference", self.reference)];
        Shape { form: "offset", values: with_curve(values, &self.base) }
    }

    fn fit(&self) -> Option<Length> {
        self.base.fit()
    }

    /// A straight base's B-spline moved square to it, which is the offset exactly; any other offset has no B-spline
    /// form.
    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        let mut form = self.base.to_bspline(lo, hi)?;
        let [from, to] = <[Point<World>; 2]>::try_from(form.poles.clone()).ok().filter(|_| form.degree == 1)?;
        let shift = self.across(from.to_vec3(), to.to_vec3()) * self.distance;
        let moved = |p: Point<World>| Point::from_vec3(p.to_vec3() + shift);
        let finite = shift.x.is_finite() && shift.y.is_finite() && shift.z.is_finite();
        form.poles = vec![moved(from), moved(to)];
        finite.then_some(form)
    }
}

#[cfg(test)]
mod tests {
    use occam_report::Code;
    use occam_units::Vec3;

    use super::*;
    use crate::conformance::{self, Check, Fitted, WrongSecondCurve, local, placement, twist};
    use crate::curve::Curve;
    use crate::{Circle, Ellipse, Hyperbola, Line, Parabola};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    /// The Z axis of [`placement`], square to the plane every planar curve here lies in.
    fn up() -> Direction<World> {
        placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)))
    }

    fn moved<C: ParametricCurve>(base: C, distance: f64) -> OffsetCurve<C> {
        OffsetCurve::new(base, mm(distance), up()).unwrap()
    }

    fn circle() -> Circle {
        Circle::placed(&placement(), mm(10.0)).unwrap()
    }

    fn line() -> Line {
        Line::new(
            Point::from_vec3(local(1.0, 2.0, 0.0)),
            placement().direction(Direction::normalised(Vec3::new(1.0, -2.0, 0.0))),
        )
    }

    fn passes(c: &dyn Curve, (lo, hi): (f64, f64)) {
        let failures = conformance::curve(c, lo, hi);
        assert!(failures.is_empty(), "{:?} from {lo} to {hi}: {failures:#?}", c.shape().values);
    }

    #[test]
    fn the_offset_of_every_curve_family_passes_the_suite() {
        passes(&moved(line(), 1.5), (-4.0, 7.5));
        passes(&moved(line().trimmed(-2.0, 3.0).unwrap(), -0.5), (-2.0, 3.0));
        passes(&moved(circle(), 2.0), (0.0, std::f64::consts::TAU));
        passes(&moved(circle().trimmed(5.0, 7.0).unwrap(), -3.0), (5.0, 7.0));
        passes(&moved(Ellipse::placed(&placement(), mm(7.0), mm(3.0)).unwrap(), 0.5), (-1.0, 4.0));
        passes(&moved(Hyperbola::placed(&placement(), mm(2.0), mm(5.0)).unwrap(), -1.0), (-1.5, 1.2));
        passes(&moved(Parabola::placed(&placement(), mm(1.5)).unwrap(), 0.6), (-4.0, 3.0));
        let range = Curve::range(&twist());
        passes(&moved(twist(), 0.2), (range.lo, range.hi));
    }

    #[test]
    fn a_box_grows_only_square_to_the_reference() {
        // A circle of radius 2 in the plane z = 0, offset by 3 in that plane: radius 5, and a box no taller than rounding.
        let flat = Circle::placed(&occam_units::Placement::identity(), mm(2.0)).unwrap();
        let square = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        let b = Curve::bounds(&OffsetCurve::new(flat, mm(3.0), square).unwrap(), 0.0, std::f64::consts::TAU);
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        assert!(low.z.abs() < 1e-12 && high.z.abs() < 1e-12, "{b:?}");
        assert!(low.x <= -5.0 && low.x > -5.0 - 1e-12 && high.y >= 5.0 && high.y < 5.0 + 1e-12, "{b:?}");
    }

    #[test]
    fn the_suite_catches_an_offset_curve_with_a_wrong_second_derivative() {
        let failed = conformance::checks(&conformance::curve(&WrongSecondCurve(moved(circle(), 2.0)), 0.0, 3.0));
        assert_eq!(failed, [Check::Differences]);
    }

    #[test]
    fn a_circle_moved_is_the_circle_about_it_and_a_line_moved_is_the_line_beside_it() {
        let centre = local(0.0, 0.0, 0.0);
        for (distance, radius) in [(2.0, 12.0), (-3.0, 7.0)] {
            let c = moved(circle(), distance);
            for t in [0.0, 1.1, 2.5, 4.0] {
                let gap = (c.point(t).to_vec3() - centre).norm();
                assert!((gap - radius).abs() < 1e-12, "{distance} at {t}: {gap}");
            }
        }
        let beside = moved(line(), 1.5);
        let (from, along) =
            (local(1.0, 2.0, 0.0), placement().direction(Direction::normalised(Vec3::new(1.0, -2.0, 0.0))).get());
        for t in [-3.0, 0.0, 5.5] {
            let off = beside.point(t).to_vec3() - from;
            let across = off - along * off.dot(along);
            assert!((across.norm() - 1.5).abs() < 1e-12 && off.dot(up().get()).abs() < 1e-12, "{t}: {off:?}");
        }
        let form = Curve::to_bspline(&beside, -1.0, 2.0).unwrap();
        assert_eq!((form.degree, form.knots.clone()), (1, vec![-1.0, -1.0, 2.0, 2.0]));
        assert!((form.poles[0].to_vec3() - beside.point(-1.0).to_vec3()).norm() < 1e-12);
        assert!(conformance::presented_curve(&beside, -1.0, 2.0).is_empty());
        assert_eq!(Curve::to_bspline(&moved(circle(), 2.0), 0.0, 1.0), None, "a circle's offset has no B-spline here");
    }

    #[test]
    fn where_the_curve_runs_along_the_reference_the_offset_is_undefined() {
        let along = Line::new(Point::from_vec3(local(0.0, 0.0, 0.0)), up());
        let c = moved(along, 1.0);
        assert!(c.point(0.5).to_vec3().x.is_nan() && c.first(0.5).1.rate().y.is_nan());
    }

    #[test]
    fn a_distance_not_finite_is_refused() {
        for bad in [f64::NAN, f64::NEG_INFINITY] {
            assert_eq!(OffsetCurve::new(circle(), mm(bad), up()).unwrap_err().code, Code::NotFinite);
        }
    }

    #[test]
    fn its_shape_content_window_and_box_name_what_defines_it() {
        let c = moved(circle(), 2.0);
        let shape = Curve::shape(&c);
        assert_eq!(
            (shape.form, shape.values[0].clone(), shape.values[1].name.as_str()),
            ("offset", NamedValue::length("distance", 2.0), "reference")
        );
        assert_eq!(shape.values[2].name, "curve-centre");
        let mut content = Content::default();
        Curve::content(&c, &mut content);
        assert_eq!(content.words()[7], 2f64.to_bits(), "the name, then the distance");
        let window = ParametricCurve::within(&moved(line(), 1.5), Point::from_vec3(local(30.0, 0.0, 0.0)), None, 2.0);
        let base = ParametricCurve::within(&line(), Point::from_vec3(local(30.0, 0.0, 0.0)), None, 3.5);
        assert_eq!(window, base, "a window for a radius larger by the distance");
        let b = Curve::bounds(&c, 0.0, 1.0);
        assert!(b.contains(c.point(0.5)) && b.contains(c.point(1.0)) && !b.contains(c.point(3.5)));
        assert_eq!(
            (Curve::family(&c), Curve::fit(&c), Curve::range(&c)),
            (FamilyName("offset"), None, Curve::range(&circle()))
        );
    }

    #[test]
    fn derivatives_of_any_order_agree_with_the_rungs() {
        let c = moved(Ellipse::placed(&placement(), mm(7.0), mm(3.0)).unwrap(), 0.5);
        let mut out = vec![];
        Curve::derivatives(&c, 0.7, 6, &mut out, &mut Scratch::default()).unwrap();
        let (_, rate, change) = c.second(0.7);
        assert_eq!((out.len(), out[0], out[1]), (6, rate, change));
        let refused = Curve::derivatives(&c, 0.7, u8::MAX, &mut out, &mut Scratch::default()).unwrap_err();
        assert_eq!(refused.code, Code::NoDerivative);
    }

    #[test]
    fn an_offset_passes_its_base_s_fit_on() {
        let x = Direction::normalised(Vec3::new(1.0, 0.0, 0.0));
        let base = Fitted(Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), x));
        let up = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        let moved = OffsetCurve::new(base, Length::from_millimetres(2.0), up).unwrap();
        assert_eq!(ParametricCurve::fit(&moved), Some(Length::from_millimetres(0.25)));
    }

    #[test]
    fn a_straight_offset_whose_shift_is_not_finite_everywhere_has_no_b_spline() {
        // From -1e308 to 1e308 the line's length overflows, and the shift square to it is NaN in some coordinates: in y
        // and z along x, and in x and y along z.
        let tilted = Direction::normalised(Vec3::new(1.0, 1.0, 1.0));
        for along in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
            let line = Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(along));
            let moved = OffsetCurve::new(line, Length::from_millimetres(1.0), tilted).unwrap();
            assert!(ParametricCurve::to_bspline(&moved, -1e308, 1e308).is_none(), "{along:?}");
            assert!(ParametricCurve::to_bspline(&moved, -1.0, 1.0).is_some(), "{along:?}");
        }
    }
}
