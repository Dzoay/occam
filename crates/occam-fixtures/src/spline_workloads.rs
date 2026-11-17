//! Source: original
//! The spline kernel `gates perf` measures: one B-spline surface at a tessellator's grid of points, at width 1 and at
//! [`width::SPLINE`](occam_math::width::SPLINE), natively by instruction count and in the browser's simd128 build by
//! time. A kernel that is not clearly faster at its width runs at width 1 (L29). Beside it, the second derivatives the
//! offsets of that surface and of [`spline_curve`] are measured at, which need their bases' third and so run a lane at a
//! time. The benchmark makes the offsets, since a family is named only in its own module.

use occam_geom::{BsplineCurve, BsplineSurface, Jets, Order, Parametric, ParametricCurve, eval_in_lanes};
use occam_math::{Level, Real};
use occam_units::{Point, UvPoint, Vec3};

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

/// The lanes of `x`, lane 0 first, and zero beyond them, as occam-geom reads them where a formula cannot run in lanes.
fn lanes<R: Real>(x: R) -> [f64; 16] {
    let mut out = [0.0; 16];
    let mut slots = out.iter_mut();
    x.for_each_lane(|value| {
        if let Some(slot) = slots.next() {
            *slot = value;
        }
    });
    out
}

/// Each lane's `x + y + z`, lane 0 first.
pub(crate) fn lane_sums<R: Real>(v: Vec3<R>) -> impl Iterator<Item = f64> {
    lanes(v.x).into_iter().zip(lanes(v.y)).zip(lanes(v.z)).map(|((x, y), z)| x + y + z)
}

/// The sum of every number a batch wrote, to show that two runs gave the same bits.
#[must_use]
pub fn total(out: &Jets) -> f64 {
    let points = out.points.iter().map(|p| p.to_vec3());
    let rates = [&out.du, &out.dv, &out.duu, &out.duv, &out.dvv].into_iter().flatten().map(|t| t.rate());
    points.chain(rates).map(|v| v.x + v.y + v.z).sum()
}
