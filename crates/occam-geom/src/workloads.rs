//! Source: original
//! The spline kernel `gates perf` measures: one B-spline surface at a tessellator's grid of points, at width 1 and at
//! [`width::SPLINE`](occam_math::width::SPLINE), natively by instruction count and in the browser's simd128 build by
//! time. A kernel that is not clearly faster at its width runs at width 1 (L29). Beside it, the second derivatives the
//! offsets of that surface and of [`spline_curve`] are measured at, which need their bases' third and so run a lane at a
//! time. The benchmark makes the offsets, since a family is named only in its own module.

use occam_math::{Level, Real};
use occam_units::{Point, UvPoint, Vec3};

use crate::batch::{Jets, Order, eval_in_lanes};
use crate::bspline::{BsplineCurve, BsplineSurface};
use crate::curve::ParametricCurve;
use crate::lanewise::lanes;
use crate::surface::Parametric;

/// How many poles in each parameter of [`spline`], and along [`spline_curve`].
const POLES: u32 = 7;

/// Cubic, clamped, over 4 spans of the unit interval.
fn knots() -> Vec<f64> {
    vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0]
}

/// The surface measured: bicubic, polynomial, with 7 by 7 poles over 4 by 4 spans of the unit square, its heights
/// varied so that nothing cancels.
#[must_use]
pub fn spline() -> BsplineSurface {
    let knots = knots();
    let height = |i: u32, j: u32| ((f64::from(i) * 7.0 + f64::from(j) * 3.0) % 5.0) * 0.4 - 0.8;
    let pole = |i: u32, j: u32| {
        let (x, y) = (f64::from(i), f64::from(j));
        Point::from_vec3(Vec3::new(1.7 * x + 0.1 * y * y, 1.3 * y - 0.2 * x, height(i, j)))
    };
    let poles = (0..POLES).flat_map(|j| (0..POLES).map(move |i| pole(i, j))).collect();
    BsplineSurface {
        degree: (3, 3),
        count: (POLES, POLES),
        knots_u: knots.clone(),
        knots_v: knots,
        poles,
        weights: None,
    }
}

/// The curve measured: cubic, polynomial, in the plane z = 0, with 7 poles over 4 spans of the unit interval, its
/// heights varied so that its third derivative, which its offset's second needs, is not zero.
#[must_use]
pub fn spline_curve() -> BsplineCurve {
    let pole = |i: u32| {
        let x = f64::from(i);
        Point::from_vec3(Vec3::new(1.5 * x, ((x * 3.0) % 5.0) * 0.6 - 1.2, 0.0))
    };
    BsplineCurve { degree: 3, knots: knots(), poles: (0..POLES).map(pole).collect(), weights: None }
}

/// `n` parameters over the unit interval, ends included.
#[must_use]
pub fn params(n: u32) -> Vec<f64> {
    let step = 1.0 / f64::from(n.saturating_sub(1).max(1));
    (0..n).map(|i| f64::from(i) * step).collect()
}

/// `n` by `n` points over the unit square, edges included, `u` changing fastest: the order a tessellator asks in, where
/// neighbours share a span.
#[must_use]
pub fn grid(n: u32) -> Vec<UvPoint> {
    let step = 1.0 / f64::from(n.saturating_sub(1).max(1));
    (0..n).flat_map(|j| (0..n).map(move |i| UvPoint::new(f64::from(i) * step, f64::from(j) * step))).collect()
}

/// The points and first derivatives of `surface` at `uv`, `W` at a time, at the baseline level so that the count does
/// not depend on the machine.
pub fn splines<const W: usize, S: Parametric>(surface: &S, uv: &[UvPoint], out: &mut Jets) {
    eval_in_lanes::<W, S>(surface, uv, Order::First, out, Level::baseline());
}

/// The points and first and second derivatives of `surface` at `uv`, `W` at a time, at the baseline level.
pub fn seconds<const W: usize, S: Parametric>(surface: &S, uv: &[UvPoint], out: &mut Jets) {
    eval_in_lanes::<W, S>(surface, uv, Order::Second, out, Level::baseline());
}

/// The sum of the points and first and second derivatives of `curve` at `ts`, `R::LANES` at a time, added in the
/// parameters' order whatever the width; a short last group repeats its last parameter, and only its own lanes count.
#[must_use]
pub fn curve_seconds<R: Real, C: ParametricCurve>(curve: &C, ts: &[f64]) -> f64 {
    let mut sum = 0.0;
    for group in ts.chunks(R::LANES) {
        let mut each = group.iter().chain(std::iter::repeat(group.last().unwrap_or(&f64::NAN)));
        let t = R::from_fn(|| each.next().copied().unwrap_or(f64::NAN));
        let (point, rate, change) = curve.eval_second(t);
        let (p, d1, d2) = (lane_sums(point.to_vec3()), lane_sums(rate.rate()), lane_sums(change.rate()));
        for ((p, d1), d2) in p.zip(d1).zip(d2).take(group.len()) {
            sum += p;
            sum += d1;
            sum += d2;
        }
    }
    sum
}

/// Each lane's `x + y + z`, lane 0 first.
fn lane_sums<R: Real>(v: Vec3<R>) -> impl Iterator<Item = f64> {
    lanes(v.x).into_iter().zip(lanes(v.y)).zip(lanes(v.z)).map(|((x, y), z)| x + y + z)
}

/// The sum of every number a batch wrote, to show that two runs gave the same bits.
#[must_use]
pub fn total(out: &Jets) -> f64 {
    let points = out.points.iter().map(|p| p.to_vec3());
    let rates = [&out.du, &out.dv, &out.duu, &out.duv, &out.dvv].into_iter().flatten().map(|t| t.rate());
    points.chain(rates).map(|v| v.x + v.y + v.z).sum()
}

#[cfg(test)]
mod tests {
    use occam_units::{Direction, Length};

    use super::*;
    use crate::offset_curve::OffsetCurve;
    use crate::offset_surface::OffsetSurface;
    use crate::spline_curve::SplineCurve;
    use crate::spline_surface::SplineSurface;

    /// The offsets the benchmark measures: the curve moved within its plane and the surface along its normal, by 0.3.
    fn offsets() -> (OffsetCurve<SplineCurve>, OffsetSurface<SplineSurface>) {
        let distance = Length::from_millimetres(0.3);
        let up = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
        let curve = OffsetCurve::new(SplineCurve::new(&spline_curve()).unwrap(), distance, up).unwrap();
        (curve, OffsetSurface::new(SplineSurface::new(&spline()).unwrap(), distance).unwrap())
    }

    #[test]
    fn the_measured_surface_and_its_sum_are_what_they_say() {
        let form = spline();
        let pole = |i: usize, j: usize| form.poles[j * 7 + i].to_vec3();
        // (1.7 x + 0.1 y^2, 1.3 y - 0.2 x, ((7 x + 3 y) mod 5) 0.4 - 0.8) at (2, 3) and at (4, 6).
        for (got, want) in [(pole(2, 3), Vec3::new(4.3, 3.5, 0.4)), (pole(4, 6), Vec3::new(10.4, 7.0, -0.4))] {
            assert!((got - want).norm() < 1e-12, "{got:?} against {want:?}");
        }
        assert_eq!((form.degree, form.count, form.knots_u.len(), form.weights), ((3, 3), (7, 7), 11, None));
        let mut out = Jets::default();
        out.points.push(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
        out.du.push(occam_units::Tangent::new(Vec3::new(4.0, 5.0, 6.0)));
        out.dvv.push(occam_units::Tangent::new(Vec3::new(7.0, 8.0, 10.0)));
        assert_eq!(total(&out), 6.0 + 15.0 + 25.0);
    }

    #[test]
    fn the_offsets_give_the_same_bits_at_either_width_and_their_parameters_are_even() {
        let (curve, surface) = offsets();
        let ts = params(9);
        assert_eq!((ts.len(), ts[0], ts[4], ts[8]), (9, 0.0, 0.5, 1.0));
        let (one, wide) = (curve_seconds::<f64, _>(&curve, &ts), curve_seconds::<occam_math::Lanes<4>, _>(&curve, &ts));
        assert!(one.is_finite() && one != 0.0, "{one}");
        assert_eq!(one.to_bits(), wide.to_bits());
        let first = curve_seconds::<f64, _>(&curve, &ts[..1]);
        let (p, d1, d2) = ParametricCurve::eval_second(&curve, 0.0);
        let want = [p.to_vec3(), d1.rate(), d2.rate()].iter().map(|v| v.x + v.y + v.z).sum::<f64>();
        assert_eq!(first.to_bits(), want.to_bits());
        // The measured curve lies in z = 0, so each lane's sum is checked where z is not zero too.
        let v = Vec3::new(
            occam_math::Lanes::new([1.0, 8.0]),
            occam_math::Lanes::new([2.0, 16.0]),
            occam_math::Lanes::new([4.0, 32.0]),
        );
        assert_eq!(lane_sums(v).take(2).collect::<Vec<_>>(), [7.0, 56.0]);
        let uv = grid(5);
        let (mut one, mut wide) = (Jets::default(), Jets::default());
        seconds::<1, _>(&surface, &uv, &mut one);
        seconds::<4, _>(&surface, &uv, &mut wide);
        assert_eq!((one.points.len(), one.duu.len()), (25, 25));
        assert!(total(&one).is_finite() && one.first_undefined().is_none());
        assert_eq!(total(&one).to_bits(), total(&wide).to_bits());
    }

    #[test]
    fn the_measured_curve_bends_in_its_third_derivative() {
        let form = spline_curve();
        assert_eq!((form.degree, form.poles.len(), form.knots.len()), (3, 7, 11));
        // (1.5 x, ((3 x) mod 5) 0.6 - 1.2, 0) at x = 2 and x = 4.
        assert_eq!(
            (form.poles[2].to_vec3(), form.poles[4].to_vec3()),
            (Vec3::new(3.0, -0.6, 0.0), Vec3::new(6.0, 0.0, 0.0))
        );
        let mut rates = Vec::new();
        let base = SplineCurve::new(&form).unwrap();
        ParametricCurve::derivatives(&base, 0.6, 3, &mut rates, &mut crate::batch::Scratch::default()).unwrap();
        assert!(rates[2].rate().norm() > 1.0, "{:?}", rates[2]);
    }

    #[test]
    fn the_kernel_gives_the_same_bits_at_either_width() {
        let surface = SplineSurface::new(&spline()).unwrap();
        let uv = grid(16);
        let (mut one, mut wide) = (Jets::default(), Jets::default());
        splines::<1, _>(&surface, &uv, &mut one);
        splines::<4, _>(&surface, &uv, &mut wide);
        assert_eq!((one.points.len(), one.du.len(), one.duu.len()), (256, 256, 0));
        assert_eq!(total(&one).to_bits(), total(&wide).to_bits());
        assert!(total(&one).is_finite() && one.first_undefined().is_none());
        assert_eq!(
            (uv[0], uv[17], uv[255]),
            (UvPoint::new(0.0, 0.0), UvPoint::new(1.0 / 15.0, 1.0 / 15.0), UvPoint::new(1.0, 1.0))
        );
    }
}
