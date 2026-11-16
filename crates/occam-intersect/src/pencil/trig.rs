//! Source: original
//! Trigonometric polynomials in the angle `θ` about a ruled quadric's axis, `a₀ + Σ aₖ cos kθ + bₖ sin kθ`, which is
//! what the other quadric leaves along the rulings. Products follow from `e^{ijθ} e^{ikθ} = e^{i(j+k)θ}`, and the roots
//! on the circle are the real roots of a polynomial in `t = tan((θ − θ₀)/2)`, with `θ₀` chosen so that `θ₀ + π`, where
//! `t` is infinite, is no root. Each is then refined by Newton's method on the angle itself.

use std::f64::consts::{FRAC_PI_4, PI, TAU};

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

    /// The largest coefficient's size.
    pub(super) fn size(&self) -> f64 {
        self.0.iter().fold(0.0, |a, c| if c.abs() > a { c.abs() } else { a })
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

    /// Its roots in `[0, 2π)`, ascending, refined by Newton's method; none where every coefficient is below `small`.
    pub(super) fn roots(&self, small: f64) -> Vec<f64> {
        if self.size() <= small {
            return Vec::new();
        }
        let candidates = (0..8_u32).map(|j| FRAC_PI_4 * f64::from(j));
        let from = candidates.fold((0.0, -1.0), |best, f| {
            let far = self.at(f + PI).abs();
            if far > best.1 { (f, far) } else { best }
        });
        let rate = self.rate();
        let mut roots: Vec<f64> = real_roots(&self.polynomial(from.0))
            .into_iter()
            .map(|t| {
                let mut theta = from.0 + 2.0 * atan2(t, 1.0);
                for _ in 0..3 {
                    let slope = rate.at(theta);
                    if slope != 0.0 {
                        theta -= self.at(theta) / slope;
                    }
                }
                theta.rem_euclid(TAU)
            })
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
        // cos 2θ − 0.28 has four roots; sin θ − 1 has one double root, which touches and is not found; 1 + cos θ has
        // one at π, where `t` would be infinite but for the choice of θ₀.
        let four = trig(&[-0.28, 0.0, 0.0, 1.0, 0.0]).roots(1e-15);
        assert_eq!(four.len(), 4, "{four:?}");
        for r in &four {
            assert!((sin_cos(2.0 * r).1 - 0.28).abs() < 1e-14, "{r}");
        }
        let shifted = trig(&[0.1, 0.8, -0.3]).roots(1e-15);
        assert_eq!(shifted.len(), 2);
        assert!(shifted.iter().all(|r| trig(&[0.1, 0.8, -0.3]).at(*r).abs() < 1e-15), "{shifted:?}");
        assert_eq!(trig(&[0.0, 0.0, 0.0]).roots(1e-15), [] as [f64; 0], "no roots of nothing");
        assert_eq!(trig(&[2.0, 1.0, 0.5]).roots(1e-15), [] as [f64; 0]);
    }
}
