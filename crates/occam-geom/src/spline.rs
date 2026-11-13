//! Source: piegl-tiller-1997
//! What the spline families share: the derivatives of a rational curve or surface from those of its weighted form, by
//! Leibniz's rule (Piegl and Tiller, A4.2 and A4.4), where the order of a surface's partial derivatives is by degree and
//! then from all `u` to all `v`; and how many functions of the basis an evaluation on the stack makes room for.

use occam_math::Real;
use occam_units::Vec3;

use crate::basis::binomial;

/// Room on the stack for the basis functions of each degree up to 3, up to 7, and up to [`crate::bspline::MAX_DEGREE`]:
/// the arrays an evaluation zeroes grow with the degree that needs them, not with the highest there can be.
pub(crate) const CAPACITIES: [usize; 3] = [4, 8, 26];

/// Where the partial derivative `a` times in `u` and `b` in `v` comes, by degree and then from all `u` to all `v`.
pub(crate) const fn index(a: usize, b: usize) -> usize {
    let degree = a.saturating_add(b);
    let pairs = degree.saturating_mul(degree.saturating_add(1));
    (pairs >> 1).saturating_add(b)
}

/// How many partial derivatives of a surface there are of total degree 0 to `order`.
pub(crate) const fn triangle(order: usize) -> usize {
    index(0, order).saturating_add(1)
}

/// `v`, each part divided by `w`.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn divided<R: Real>(v: Vec3<R>, w: R) -> Vec3<R> {
    Vec3::new(v.x / w, v.y / w, v.z / w)
}

/// Replaces the derivatives of a curve's weighted form, `out[k]` for order `k` from 0 to `order`, with the curve's, given
/// the weight's in `weights`: `C(k) = (A(k) - sum over i from 1 to k of (k choose i) w(i) C(k - i)) / w(0)`.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn curve_quotient<R: Real>(order: usize, out: &mut [Vec3<R>], weights: &[R]) {
    let Some(&w) = weights.first() else { return };
    for k in 0..=order {
        let Some(mut v) = out.get(k).copied() else { return };
        for i in 1..=k {
            if let (Some(&wi), Some(&lower)) = (weights.get(i), out.get(k.saturating_sub(i))) {
                v = v - lower * (R::splat(binomial(k, i)) * wi);
            }
        }
        if let Some(slot) = out.get_mut(k) {
            *slot = divided(v, w);
        }
    }
}

/// Replaces the partial derivatives of a surface's weighted form to `order`, at [`index`] in `out`, with the
/// surface's, given the weight's at the same places in `weights`: `S(a, b)` is `A(a, b)` less `(a choose i) (b choose
/// j) w(i, j) S(a - i, b - j)` for every other `i` to `a` and `j` to `b`, over `w(0, 0)`, each from those of lower
/// degree, which come first.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn surface_quotient<R: Real>(order: usize, out: &mut [Vec3<R>], weights: &[R]) {
    let Some(&w) = weights.first() else { return };
    for degree in 0..=order {
        for b in 0..=degree {
            let a = degree.saturating_sub(b);
            let Some(mut v) = out.get(index(a, b)).copied() else { return };
            for i in 0..=a {
                for j in (0..=b).filter(|j| i > 0 || *j > 0) {
                    let lower = out.get(index(a.saturating_sub(i), b.saturating_sub(j)));
                    if let (Some(&wij), Some(&lower)) = (weights.get(index(i, j)), lower) {
                        v = v - lower * (R::splat(binomial(a, i) * binomial(b, j)) * wij);
                    }
                }
            }
            if let Some(slot) = out.get_mut(index(a, b)) {
                *slot = divided(v, w);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_derivatives_come_by_degree_then_from_u_to_v() {
        let order: Vec<(usize, usize)> = (0..=3).flat_map(|d| (0..=d).map(move |b| (d - b, b))).collect();
        let indices: Vec<usize> = order.iter().map(|(a, b)| index(*a, *b)).collect();
        assert_eq!(indices, (0..10).collect::<Vec<_>>());
        assert_eq!([triangle(0), triangle(1), triangle(2), triangle(3)], [1, 3, 6, 10]);
    }

    #[test]
    fn a_quotient_undoes_a_weight() {
        // The weighted form of c(t) = (t, t^2, 1) under w(t) = 1 + t is ((1 + t) t, (1 + t) t^2, 1 + t).
        let t: f64 = 0.5;
        let weighted = [
            Vec3::new((1.0 + t) * t, (1.0 + t) * t * t, 1.0 + t),
            Vec3::new(1.0 + 2.0 * t, 2.0 * t + 3.0 * t * t, 1.0),
            Vec3::new(2.0, 2.0 + 6.0 * t, 0.0),
        ];
        let mut out = weighted;
        curve_quotient(2, &mut out, &[1.0 + t, 1.0, 0.0]);
        for (got, want) in
            out.iter().zip([Vec3::new(t, t * t, 1.0), Vec3::new(1.0, 2.0 * t, 0.0), Vec3::new(0.0, 2.0, 0.0)])
        {
            assert!((*got - want).norm() < 1e-15, "{got:?} {want:?}");
        }
    }

    #[test]
    fn a_surface_quotient_undoes_a_weight() {
        // s(u, v) = (u, v, u v) under w = 1 + u + 2 v, at (0.5, 0.25): A = w s, and its partials to the second.
        let (u, v): (f64, f64) = (0.5, 0.25);
        let w = 1.0 + u + 2.0 * v;
        let a = |s: [f64; 3]| Vec3::new(s[0], s[1], s[2]);
        let weighted = [
            a([w * u, w * v, w * u * v]),
            a([w + u, v, w * v + u * v]),
            a([2.0 * u, w + 2.0 * v, w * u + 2.0 * u * v]),
            a([2.0, 0.0, 2.0 * v]),
            a([2.0, 1.0, w + u + 2.0 * v]),
            a([0.0, 4.0, 4.0 * u]),
        ];
        let weights = [w, 1.0, 2.0, 0.0, 0.0, 0.0];
        let mut out = weighted;
        surface_quotient(2, &mut out, &weights);
        let want = [a([u, v, u * v]), a([1.0, 0.0, v]), a([0.0, 1.0, u]), a([0.0; 3]), a([0.0, 0.0, 1.0]), a([0.0; 3])];
        for (got, want) in out.iter().zip(want) {
            assert!((*got - want).norm() < 1e-14, "{got:?} {want:?}");
        }
    }
}
