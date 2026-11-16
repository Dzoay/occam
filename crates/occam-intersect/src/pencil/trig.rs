//! Source: original
//! Trigonometric polynomials in the angle `θ` about a ruled quadric's axis, `a₀ + Σ aₖ cos kθ + bₖ sin kθ`, which is
//! what the other quadric leaves along the rulings. Products follow from `e^{ijθ} e^{ikθ} = e^{i(j+k)θ}`, and the roots
//! on the circle are the real roots of a polynomial in `t = tan((θ − θ₀)/2)`, with `θ₀` chosen so that `θ₀ + π`, where
//! `t` is infinite, is no root.

use std::f64::consts::{PI, TAU};

use occam_math::{atan2, sin_cos};

use super::poly::real_roots;

/// `a₀, a₁, b₁, a₂, b₂, …`: the constant, then the cosine and sine coefficients of each multiple of the angle.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Trig(pub(super) Vec<f64>);

/// A complex number as its two parts.
type Complex = (f64, f64);

impl Trig {
    /// The highest multiple of the angle it has a coefficient for.
    pub(super) fn degree(&self) -> usize {
        self.0.len().saturating_sub(1) / 2
    }

    /// The coefficients of `cos kθ` and `sin kθ`.
    fn pair(&self, k: usize) -> (f64, f64) {
        if k == 0 {
            return (self.0.first().copied().unwrap_or(0.0), 0.0);
        }
        let at = |i: usize| self.0.get(i).copied().unwrap_or(0.0);
        let twice = k.saturating_mul(2);
        (at(twice.saturating_sub(1)), at(twice))
    }

    /// Its value at `theta`.
    pub(super) fn at(&self, theta: f64) -> f64 {
        (0..=self.degree())
            .map(|k| {
                let (a, b) = self.pair(k);
                let (sin, cos) = sin_cos(theta * f64::from(u32::try_from(k).unwrap_or(0)));
                a * cos + b * sin
            })
            .sum()
    }

    /// Its derivative.
    pub(super) fn rate(&self) -> Self {
        let mut out = vec![0.0];
        for k in 1..=self.degree() {
            let (a, b) = self.pair(k);
            let n = f64::from(u32::try_from(k).unwrap_or(0));
            out.extend([n * b, -n * a]);
        }
        Self(out)
    }

    /// `self + other`.
    pub(super) fn plus(&self, other: &Self) -> Self {
        let n = self.0.len().max(other.0.len());
        let at = |p: &Self, i: usize| p.0.get(i).copied().unwrap_or(0.0);
        Self((0..n).map(|i| at(self, i) + at(other, i)).collect())
    }

    /// `self × by`.
    pub(super) fn scaled(&self, by: f64) -> Self {
        Self(self.0.iter().map(|c| c * by).collect())
    }

    /// The complex coefficient of `e^{ikθ}`, for `k` of either sign.
    fn complex(&self, k: i64) -> Complex {
        let (a, b) = self.pair(usize::try_from(k.unsigned_abs()).unwrap_or(usize::MAX));
        match k.signum() {
            0 => (a, 0.0),
            1 => (0.5 * a, -0.5 * b),
            _ => (0.5 * a, 0.5 * b),
        }
    }

    /// `self × other`.
    pub(super) fn times(&self, other: &Self) -> Self {
        let degree = self.degree().saturating_add(other.degree());
        let top = i64::try_from(self.degree()).unwrap_or(0);
        let mut out = vec![0.0; degree.saturating_mul(2).saturating_add(1)];
        for k in 0..=degree {
            let total = i64::try_from(k).unwrap_or(0);
            let (mut re, mut im) = (0.0, 0.0);
            for j in top.saturating_neg()..=top {
                let (one, two) = (self.complex(j), other.complex(total.saturating_sub(j)));
                re += one.0 * two.0 - one.1 * two.1;
                im += one.0 * two.1 + one.1 * two.0;
            }
            let twice = k.saturating_mul(2);
            let slots =
                if k == 0 { vec![(0, re)] } else { vec![(twice.saturating_sub(1), 2.0 * re), (twice, -2.0 * im)] };
            for (i, value) in slots {
                if let Some(slot) = out.get_mut(i) {
                    *slot = value;
                }
            }
        }
        Self(out)
    }

    /// Its coefficients' sizes, summed: a bound on its size anywhere.
    pub(super) fn size(&self) -> f64 {
        self.0.iter().map(|c| c.abs()).sum()
    }

    /// `(1 + t²)ⁿ self(θ₀ + 2 atan t)` as a polynomial in `t`, where `n` is the degree.
    fn polynomial(&self, from: f64) -> Vec<f64> {
        let n = self.degree();
        let mut out = vec![0.0; n.saturating_mul(2).saturating_add(1)];
        for k in 0..=n {
            let (a, b) = self.pair(k);
            let (sin, cos) = sin_cos(from * f64::from(u32::try_from(k).unwrap_or(0)));
            let (along, across) = (a * cos + b * sin, b * cos - a * sin);
            let turned = power_of_one_plus_it(k.saturating_mul(2));
            let spread = power_of_one_plus_t_squared(n.saturating_sub(k));
            for (i, (re, im)) in turned.iter().enumerate() {
                for (j, s) in spread.iter().enumerate() {
                    if let Some(slot) = out.get_mut(i.saturating_add(j)) {
                        *slot += (along * re + across * im) * s;
                    }
                }
            }
        }
        out
    }

    /// Its roots in `[0, 2π)` where it changes sign, ascending: those of its polynomial in the half-angle's tangent from
    /// half a turn past where it is farthest from zero of nine angles a ninth of a turn apart, where that tangent is
    /// infinite. A polynomial of degree four at most that is not zero is not zero at all nine.
    pub(super) fn roots(&self) -> Vec<f64> {
        let far = (0..9_u32).map(|k| f64::from(k) * (TAU / 9.0)).fold((0.0, -1.0), |best, far| {
            let size = self.at(far).abs();
            if size > best.1 { (far, size) } else { best }
        });
        let from = far.0 + PI;
        let mut roots: Vec<f64> = real_roots(&self.polynomial(from))
            .into_iter()
            .map(|t| (from + 2.0 * atan2(t, 1.0)).rem_euclid(TAU))
            .collect();
        roots.sort_by(f64::total_cmp);
        roots
    }
}

/// `(1 + it)ᵐ`, its coefficients from the constant up.
fn power_of_one_plus_it(m: usize) -> Vec<Complex> {
    let mut out = vec![(1.0, 0.0)];
    for _ in 0..m {
        let mut next = vec![(0.0, 0.0); out.len().saturating_add(1)];
        for (i, (re, im)) in out.iter().enumerate() {
            if let Some(slot) = next.get_mut(i) {
                *slot = (slot.0 + re, slot.1 + im);
            }
            if let Some(slot) = next.get_mut(i.saturating_add(1)) {
                *slot = (slot.0 - im, slot.1 + re);
            }
        }
        out = next;
    }
    out
}

/// `(1 + t²)ᵖ`, its coefficients from the constant up.
fn power_of_one_plus_t_squared(p: usize) -> Vec<f64> {
    let mut out = vec![1.0];
    for _ in 0..p {
        let mut next = vec![0.0; out.len().saturating_add(2)];
        for (i, c) in out.iter().enumerate() {
            for j in [i, i.saturating_add(2)] {
                if let Some(slot) = next.get_mut(j) {
                    *slot += c;
                }
            }
        }
        out = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trig(c: &[f64]) -> Trig {
        Trig(c.to_vec())
    }

    #[test]
    fn its_degree_is_the_highest_multiple_of_the_angle_it_has_a_coefficient_for() {
        assert_eq!([1, 2, 3, 5, 9].map(|n| trig(&vec![1.0; n]).degree()), [0, 0, 1, 2, 4]);
    }

    #[test]
    fn a_product_and_a_rate_agree_with_the_values() {
        let (p, q) = (trig(&[0.3, -1.0, 0.5, 0.2, 0.7]), trig(&[1.1, 0.4, -0.6]));
        let (product, rate) = (p.times(&q), p.rate());
        assert_eq!(product.degree(), 3);
        for theta in [0.0, 0.7, 2.0, 4.4, 6.1] {
            assert!((product.at(theta) - p.at(theta) * q.at(theta)).abs() < 1e-14, "{theta}");
            let h = 1e-6;
            let slope = (p.at(theta + h) - p.at(theta - h)) / (2.0 * h);
            assert!((rate.at(theta) - slope).abs() < 1e-8, "{theta}");
        }
    }

    #[test]
    fn the_roots_on_the_circle_are_found_wherever_they_are() {
        let close = |got: Vec<f64>, want: &[f64]| {
            assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
            assert!(got.iter().all(|g| (0.0..TAU).contains(g)), "{got:?}");
            for w in want {
                let near = got.iter().any(|g| sin_cos(g - w).0.abs() <= 1e-15 && sin_cos(g - w).1 > 0.0);
                assert!(near, "{got:?} against {want:?}");
            }
        };
        // cos 2θ − 0.28: four roots, a quarter turn of 2θ apart about where cos 2θ = 0.28.
        let h = 0.5 * atan2((1.0_f64 - 0.28 * 0.28).sqrt(), 0.28);
        close(trig(&[-0.28, 0.0, 0.0, 1.0, 0.0]).roots(), &[h, PI - h, PI + h, TAU - h]);
        // sin θ, with a root at 0 and at π, where the first angle tried would put `t` at infinity; and cos θ.
        close(trig(&[0.0, 0.0, 1.0]).roots(), &[0.0, PI]);
        close(trig(&[0.0, 1.0, 0.0]).roots(), &[0.5 * PI, 1.5 * PI]);
        // 10 + 10 cos θ + sin θ, farthest from zero at 0, where `t` is infinite: a root at π, and where tan θ/2 = −10.
        close(trig(&[10.0, 10.0, 1.0]).roots(), &[PI, TAU - 2.0 * atan2(10.0, 1.0)]);
        // (cos θ − cos 2θ + sin θ / 4) / 4, no more than 1 anywhere, whose root at 0 a start half a turn from 0 would
        // lose: its polynomial's leading coefficient, its value there, would be exactly zero, the sine's part rounded away.
        let p = trig(&[0.0, 0.25, 0.0625, -0.25, 0.0]);
        let changes = (0..10_000).filter(|i| {
            let at = |k: i32| p.at(TAU * f64::from(k) / 10_000.0 - 1e-9);
            at(*i) * at(i + 1) < 0.0
        });
        let got = p.roots();
        assert_eq!(got.len(), changes.count(), "{got:?}");
        assert!(
            got.iter().all(|r| p.at(*r).abs() <= 1e-14) && got.iter().any(|r| sin_cos(*r).0.abs() <= 1e-15),
            "{got:?}"
        );
        // sin 4θ is zero at every multiple of a quarter of a half turn, but not at every ninth of a turn.
        let eighths: Vec<f64> = (0..8).map(|k| PI * f64::from(k) / 4.0).collect();
        close(trig(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]).roots(), &eighths);
        let shifted = trig(&[0.1, 0.8, -0.3]).roots();
        assert_eq!(shifted.len(), 2);
        assert!(shifted.iter().all(|r| trig(&[0.1, 0.8, -0.3]).at(*r).abs() < 1e-15), "{shifted:?}");
        assert_eq!(trig(&[0.0, 0.0, 0.0]).roots(), [] as [f64; 0], "no roots of nothing");
        assert_eq!(trig(&[2.0, 1.0, 0.5]).roots(), [] as [f64; 0]);
    }

    #[test]
    fn its_polynomial_in_the_half_angle_is_its_value_scaled() {
        let p = trig(&[0.3, -1.0, 0.5, 0.2, 0.7]);
        for from in [0.0, 0.9, 2.5] {
            let poly = p.polynomial(from);
            for t in [-3.0, -0.4, 0.0, 0.7, 5.0] {
                let scale = (1.0 + t * t) * (1.0 + t * t);
                let want = scale * p.at(from + 2.0 * atan2(t, 1.0));
                let got = super::super::poly::at(&poly, t);
                assert!((got - want).abs() <= 1e-12 * scale, "{from} {t}: {got} against {want}");
            }
        }
        assert!((p.size() - 2.7).abs() <= 1e-15);
    }
}
