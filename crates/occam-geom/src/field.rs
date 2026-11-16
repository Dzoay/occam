//! Source: original
//! [`Field`]: the arithmetic a closed form is written over once, so that one formula gives a point and its first two
//! rates in lanes ([`Jet`]), its rates to any order as Taylor series ([`Taylor`]), and a box holding it over a stretch
//! of its parameter ([`Interval`]). Only `+ − × ÷`, the square root and the owned `sin_cos` are used, so no new
//! transcendental function enters.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use occam_math::{Interval, Real, sin_cos};

use crate::series;
use crate::support::span;

/// What a closed form is computed in.
pub(crate) trait Field: Clone {
    /// The constant `x`, of the same kind as `self`.
    fn constant(&self, x: f64) -> Self;
    /// `self + other`.
    fn plus(&self, other: &Self) -> Self;
    /// `self − other`.
    fn minus(&self, other: &Self) -> Self;
    /// `self × other`.
    fn times(&self, other: &Self) -> Self;
    /// `self ÷ other`.
    fn over(&self, other: &Self) -> Self;
    /// `self × by`.
    fn scaled(&self, by: f64) -> Self;
    /// The square root.
    fn root(&self) -> Self;
    /// The sine and the cosine.
    fn sin_cos(&self) -> (Self, Self);
}

/// A value and its first two rates with respect to one parameter, in lanes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Jet<R> {
    pub(crate) value: R,
    pub(crate) first: R,
    pub(crate) second: R,
}

impl<R: Real> Jet<R> {
    /// The parameter itself at `t`.
    pub(crate) fn variable(t: R) -> Self {
        Self { value: t, first: R::splat(1.0), second: R::splat(0.0) }
    }
}

#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
impl<R: Real> Field for Jet<R> {
    fn constant(&self, x: f64) -> Self {
        Self { value: R::splat(x), first: R::splat(0.0), second: R::splat(0.0) }
    }

    fn plus(&self, o: &Self) -> Self {
        Self { value: self.value + o.value, first: self.first + o.first, second: self.second + o.second }
    }

    fn minus(&self, o: &Self) -> Self {
        Self { value: self.value - o.value, first: self.first - o.first, second: self.second - o.second }
    }

    fn times(&self, o: &Self) -> Self {
        let first = self.first * o.value + self.value * o.first;
        let second = self.second * o.value + R::splat(2.0) * self.first * o.first + self.value * o.second;
        Self { value: self.value * o.value, first, second }
    }

    fn over(&self, o: &Self) -> Self {
        let value = self.value / o.value;
        let first = (self.first - value * o.first) / o.value;
        let second = (self.second - R::splat(2.0) * first * o.first - value * o.second) / o.value;
        Self { value, first, second }
    }

    fn scaled(&self, by: f64) -> Self {
        let k = R::splat(by);
        Self { value: self.value * k, first: self.first * k, second: self.second * k }
    }

    fn root(&self) -> Self {
        let value = self.value.sqrt();
        let first = self.first / (R::splat(2.0) * value);
        let second = (self.second - R::splat(2.0) * first * first) / (R::splat(2.0) * value);
        Self { value, first, second }
    }

    fn sin_cos(&self) -> (Self, Self) {
        let (sin, cos) = sin_cos(self.value);
        let (f, s) = (self.first, self.second);
        let up = Self { value: sin, first: cos * f, second: cos * s - sin * f * f };
        let across = Self { value: cos, first: -(sin * f), second: -(cos * f * f) - sin * s };
        (up, across)
    }
}

/// A truncated Taylor series, `f_k = f⁽ᵏ⁾(t) / k!` from `k = 0`.
#[derive(Clone, Debug)]
pub(crate) struct Taylor(pub(crate) Vec<f64>);

impl Taylor {
    /// The parameter itself at `t`, to order `order`.
    pub(crate) fn variable(t: f64, order: u8) -> Self {
        Self((0..=order).map(|k| [t, 1.0].get(usize::from(k)).copied().unwrap_or(0.0)).collect())
    }
}

impl Field for Taylor {
    fn constant(&self, x: f64) -> Self {
        Self(self.0.iter().enumerate().map(|(k, _)| if k == 0 { x } else { 0.0 }).collect())
    }

    fn plus(&self, o: &Self) -> Self {
        Self(series::plus(&self.0, &o.0))
    }

    fn minus(&self, o: &Self) -> Self {
        Self(series::plus(&self.0, &series::scaled(&o.0, -1.0, 0.0)))
    }

    fn times(&self, o: &Self) -> Self {
        Self(series::product(&self.0, &o.0))
    }

    fn over(&self, o: &Self) -> Self {
        Self(series::quotient(&self.0, &o.0))
    }

    fn scaled(&self, by: f64) -> Self {
        Self(series::scaled(&self.0, by, 0.0))
    }

    fn root(&self) -> Self {
        Self(series::root(&self.0))
    }

    /// From `sin' = cos θ'` and `cos' = −sin θ'`: `s_k = Σ j θ_j c_{k−j} / k`, `c_k = −Σ j θ_j s_{k−j} / k`.
    fn sin_cos(&self) -> (Self, Self) {
        let first = self.0.first().copied().unwrap_or(f64::NAN);
        let (sin, cos) = sin_cos(first);
        let (mut s, mut c) = (vec![sin], vec![cos]);
        for k in 1..self.0.len() {
            let weighted = |other: &[f64]| -> f64 {
                (1..=k)
                    .map(|j| {
                        let a = self.0.get(j).copied().unwrap_or(0.0);
                        let b = other.get(k.saturating_sub(j)).copied().unwrap_or(0.0);
                        f64::from(u32::try_from(j).unwrap_or(u32::MAX)) * a * b
                    })
                    .sum()
            };
            let order = f64::from(u32::try_from(k).unwrap_or(u32::MAX));
            let (ds, dc) = (weighted(&c) / order, -weighted(&s) / order);
            s.push(ds);
            c.push(dc);
        }
        (Self(s), Self(c))
    }
}

/// Whether some `at + 2kπ` lies from `lo` to `hi`, a little widened for the rounding of `2kπ`.
fn reaches(at: f64, lo: f64, hi: f64) -> bool {
    let k = ((lo - at) / TAU).ceil();
    let first = at + k * TAU;
    first <= hi + 1e-15 * (1.0 + hi.abs()) || at + (k - 1.0) * TAU >= lo - 1e-15 * (1.0 + lo.abs())
}

/// An interval holding `values` and, where the stretch reaches them, the extremes `top` and `bottom`, padded for the
/// owned `sin_cos`, which is within an ulp.
fn holding([a, b]: [f64; 2], (top, bottom): (bool, bool)) -> Interval {
    let (least, most) = if a < b { (a, b) } else { (b, a) };
    let low = if bottom { -1.0 } else { least };
    let high = if top { 1.0 } else { most };
    let pad = 4.0 * f64::EPSILON;
    span(low - pad, high + pad)
}

impl Field for Interval {
    fn constant(&self, x: f64) -> Self {
        Self::exact(x)
    }

    fn plus(&self, o: &Self) -> Self {
        *self + *o
    }

    fn minus(&self, o: &Self) -> Self {
        *self - *o
    }

    fn times(&self, o: &Self) -> Self {
        *self * *o
    }

    fn over(&self, o: &Self) -> Self {
        *self / *o
    }

    fn scaled(&self, by: f64) -> Self {
        *self * Self::exact(by)
    }

    fn root(&self) -> Self {
        self.sqrt()
    }

    /// The sine and cosine of every value held: their values at the ends, and ±1 where the stretch reaches a peak.
    fn sin_cos(&self) -> (Self, Self) {
        let (lo, hi) = (self.lower(), self.upper());
        if !(lo.is_finite() && hi.is_finite()) || hi - lo >= TAU {
            let all = span(-1.0, 1.0);
            return (all, all);
        }
        let ((s0, c0), (s1, c1)) = (sin_cos(lo), sin_cos(hi));
        let sin = holding([s0, s1], (reaches(FRAC_PI_2, lo, hi), reaches(-FRAC_PI_2, lo, hi)));
        let cos = holding([c0, c1], (reaches(0.0, lo, hi), reaches(PI, lo, hi)));
        (sin, cos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sin(t²) / (1 + t)`, `√(2 + cos t)` and their sum, in any field.
    fn formula<F: Field>(t: &F) -> F {
        let one = t.constant(1.0);
        let (sin, _) = t.times(t).sin_cos();
        let (_, cos) = t.sin_cos();
        sin.over(&one.plus(t)).plus(&cos.plus(&t.constant(2.0)).root()).minus(&t.scaled(0.5))
    }

    #[test]
    fn a_jet_and_a_series_agree_with_differences_of_the_same_formula() {
        for t in [-0.4, 0.3, 1.7] {
            let jet = formula(&Jet::variable(t));
            let series = formula(&Taylor::variable(t, 4));
            let value = |x: f64| formula(&Jet::variable(x)).value;
            let h = 1e-4;
            let first = (value(t + h) - value(t - h)) / (2.0 * h);
            let second = (value(t + h) - 2.0 * value(t) + value(t - h)) / (h * h);
            assert!((jet.first - first).abs() < 1e-7 && (jet.second - second).abs() < 1e-5, "{t}: {jet:?}");
            let got = series::derivatives(&series.0);
            assert_eq!(series.0.first().copied(), Some(jet.value));
            assert!((got[0] - jet.first).abs() < 1e-13 && (got[1] - jet.second).abs() < 1e-12, "{got:?}");
            let third = (formula(&Jet::variable(t + h)).second - formula(&Jet::variable(t - h)).second) / (2.0 * h);
            assert!((got[2] - third).abs() < 1e-5, "{t}: {} against {third}", got[2]);
        }
    }

    #[test]
    fn an_interval_holds_the_formula_over_a_stretch() {
        for (lo, hi) in [(-0.5, 0.2), (1.0, 1.6), (3.0, 3.3), (-2.0, 4.6)] {
            let boxed = formula(&span(lo, hi));
            for i in 0..=200 {
                let t = lo + (hi - lo) * f64::from(i) / 200.0;
                let x = formula(&Jet::variable(t)).value;
                assert!(boxed.lower() <= x && x <= boxed.upper(), "{t}: {x} outside {boxed:?}");
            }
        }
    }
}
