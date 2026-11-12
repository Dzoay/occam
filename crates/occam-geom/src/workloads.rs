//! Source: original
//! The spline kernel `gates perf` measures: one B-spline surface at a tessellator's grid of points, at width 1 and at
//! [`width::SPLINE`](occam_math::width::SPLINE), natively by instruction count and in the browser's simd128 build by
//! time. A kernel that is not clearly faster at its width runs at width 1 (L29).

use occam_math::Level;
use occam_units::{Point, UvPoint, Vec3};

use crate::batch::{Jets, Order, eval_in_lanes};
use crate::bspline::BsplineSurface;
use crate::surface::Parametric;

/// How many poles in each parameter of [`spline`].
const POLES: u32 = 7;

/// The surface measured: bicubic, polynomial, with 7 by 7 poles over 4 by 4 spans of the unit square, its heights
/// varied so that nothing cancels.
#[must_use]
pub fn spline() -> BsplineSurface {
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0];
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

/// The sum of every number a batch wrote, to show that two runs gave the same bits.
#[must_use]
pub fn total(out: &Jets) -> f64 {
    let points = out.points.iter().map(|p| p.to_vec3());
    let rates = [&out.du, &out.dv, &out.duu, &out.duv, &out.dvv].into_iter().flatten().map(|t| t.rate());
    points.chain(rates).map(|v| v.x + v.y + v.z).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spline_surface::SplineSurface;

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
