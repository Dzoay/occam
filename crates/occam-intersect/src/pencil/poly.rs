//! Source: original
//! The real roots of a polynomial of low degree, isolated by the roots of its derivative and refined by Newton's method
//! inside each bracket, falling back to bisection where a step would leave it. Between two neighbouring roots of the
//! derivative the polynomial is monotone, so it has at most one root there, and a sign change finds it. A root of even
//! multiplicity, which touches zero without crossing, is not found here: the pencil method finds those as the critical
//! points they are, and decides them against the band.

/// `p` at `t`, by Horner's rule; `p` holds the coefficients from the constant up.
pub(super) fn at(p: &[f64], t: f64) -> f64 {
    p.iter().rev().fold(0.0, |acc, c| acc * t + c)
}

/// The coefficients of `p`'s derivative.
fn derivative(p: &[f64]) -> Vec<f64> {
    p.iter().zip(0_u32..).skip(1).map(|(c, k)| c * f64::from(k)).collect()
}

/// `p` without its highest coefficients that are zero.
fn trimmed(p: &[f64]) -> &[f64] {
    let degree = p.iter().rposition(|c| *c != 0.0).map_or(0, |i| i.saturating_add(1));
    p.get(..degree).unwrap_or(&[])
}

/// The root of `p` between `lo` and `hi`, where `p` changes sign: Newton's step where it stays inside the bracket,
/// halving it otherwise, until the bracket is as narrow as the numbers allow.
fn bracketed(p: &[f64], dp: &[f64], (mut lo, mut hi): (f64, f64)) -> f64 {
    let rising = at(p, hi) > 0.0;
    let mut t = 0.5 * lo + 0.5 * hi;
    for _ in 0..200 {
        let value = at(p, t);
        if value == 0.0 {
            return t;
        }
        if (value > 0.0) == rising {
            hi = t;
        } else {
            lo = t;
        }
        let newton = t - value / at(dp, t);
        let next = if lo < newton && newton < hi { newton } else { 0.5 * lo + 0.5 * hi };
        if next.to_bits() == t.to_bits() || hi - lo <= 4.0 * f64::EPSILON * (lo.abs() + hi.abs()) {
            return next;
        }
        t = next;
    }
    t
}

/// The real roots of `p`, ascending, each once: those where it changes sign.
pub(super) fn real_roots(p: &[f64]) -> Vec<f64> {
    let p = trimmed(p);
    let (Some(lead), degree) = (p.last().copied(), p.len().saturating_sub(1)) else { return Vec::new() };
    match degree {
        0 => return Vec::new(),
        1 => return vec![-p.first().copied().unwrap_or(0.0) / lead],
        _ => {}
    }
    let dp = derivative(p);
    let bound = 1.0 + p.iter().map(|c| (c / lead).abs()).fold(0.0, |a, x| if x > a { x } else { a });
    let mut points = vec![-bound];
    points.extend(real_roots(&dp).into_iter().filter(|t| -bound < *t && *t < bound));
    points.push(bound);
    let mut roots = Vec::new();
    for pair in points.windows(2) {
        let [lo, hi] = [pair.first().copied().unwrap_or(0.0), pair.last().copied().unwrap_or(0.0)];
        let (below, above) = (at(p, lo), at(p, hi));
        if below == 0.0 && roots.last().is_none_or(|r: &f64| *r < lo) {
            roots.push(lo);
        }
        if (below < 0.0 && above > 0.0) || (below > 0.0 && above < 0.0) {
            roots.push(bracketed(p, &dp, (lo, hi)));
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The polynomial with these roots, monic.
    fn with_roots(roots: &[f64]) -> Vec<f64> {
        roots.iter().fold(vec![1.0], |p, r| {
            let mut next = vec![0.0; p.len().saturating_add(1)];
            for (k, c) in p.iter().enumerate() {
                next[k.saturating_add(1)] += c;
                next[k] -= r * c;
            }
            next
        })
    }

    #[test]
    fn the_roots_of_a_polynomial_are_found_each_once_and_in_order() {
        let cases: [&[f64]; 4] = [&[-3.0, 0.5, 2.0], &[-2.0, -1.0, 0.25, 1.5, 7.0], &[0.1], &[-1e-3, 1e-3, 40.0, 41.0]];
        for want in cases {
            let got = real_roots(&with_roots(want));
            assert_eq!(got.len(), want.len(), "{want:?}: {got:?}");
            for (g, w) in got.iter().zip(want) {
                assert!((g - w).abs() <= 1e-12 * (1.0 + w.abs()), "{want:?}: {got:?}");
            }
        }
    }

    #[test]
    fn a_polynomial_with_no_real_root_or_no_degree_has_none() {
        let none = [] as [f64; 0];
        assert_eq!(real_roots(&[1.0, 0.0, 1.0]), none);
        assert_eq!(real_roots(&[2.0, 0.0, 3.0, 0.0, 1.0]), none);
        assert_eq!(real_roots(&[0.0, 0.0]), none);
        assert_eq!(real_roots(&[]), none);
        assert_eq!(real_roots(&[3.0, 0.0, 0.0]).len(), 0);
    }
}
