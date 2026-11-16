//! Source: original
//! The real roots of a polynomial of low degree, each isolated between neighbouring roots of its derivative, where the
//! polynomial is monotone and so changes sign at most once, then halved down to neighbouring numbers. A root of even
//! multiplicity, which touches zero without crossing, is not found here: the pencil method finds those as the critical
//! points they are, and decides them against the band.

/// Halvings of a bracket: enough to take any bracket the bound gives down to neighbouring numbers.
const HALVINGS: u32 = 200;

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

/// The root of `p` between `lo` and `hi`, at which it has opposite signs, by halving the bracket. Where `p` is zero at
/// the middle, the middle is the root, and it stays an end of the bracket whichever half is kept.
fn bracketed(p: &[f64], (mut lo, mut hi): (f64, f64)) -> f64 {
    let low_below = at(p, lo).is_sign_negative();
    for _ in 0..HALVINGS {
        let mid = 0.5 * lo + 0.5 * hi;
        if at(p, mid).is_sign_negative() == low_below {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * lo + 0.5 * hi
}

/// The real roots of `p` where it changes sign, ascending, each once. Every root lies strictly within `Σ |cᵢ / cₙ|`,
/// the leading coefficient's one included, as Cauchy's bound `1 + max |cᵢ / cₙ|` does, and so, by Gauss and Lucas,
/// does every root of the derivative.
pub(super) fn real_roots(p: &[f64]) -> Vec<f64> {
    let p = trimmed(p);
    let Some(lead) = p.last().copied() else { return Vec::new() };
    let bound = p.iter().map(|c| (c / lead).abs()).sum::<f64>();
    let points: Vec<f64> = std::iter::once(-bound).chain(real_roots(&derivative(p))).chain([bound]).collect();
    let mut roots = Vec::new();
    for pair in points.windows(2) {
        let [lo, hi] = [pair.first().copied().unwrap_or(0.0), pair.last().copied().unwrap_or(0.0)];
        if at(p, lo) * at(p, hi) < 0.0 {
            roots.push(bracketed(p, (lo, hi)));
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
    fn a_polynomial_is_its_value_by_horners_rule() {
        assert_eq!(at(&[1.0, 2.0, 3.0], 2.0), 17.0);
        assert_eq!(at(&[-4.0, 0.5], 3.0), -2.5);
        assert_eq!(at(&[], 3.0), 0.0);
    }

    #[test]
    fn a_triple_root_crosses_and_is_found_a_double_root_touches_and_is_not() {
        let triple = real_roots(&[0.0, 0.0, 0.0, 1.0]);
        assert!(triple.len() == 1 && triple[0].abs() <= 1e-15, "t³: {triple:?}");
        // (t − 1)²: its derivative's root is exactly 1, where it touches zero, which is no change of sign.
        assert_eq!(real_roots(&[1.0, -2.0, 1.0]), [] as [f64; 0]);
        // (t − 1)² (t + 2): only the simple root.
        let simple = real_roots(&[2.0, -3.0, 0.0, 1.0]);
        assert!(simple.len() == 1 && (simple[0] + 2.0).abs() <= 1e-15, "{simple:?}");
        // Negated, it is negative before its double root, where it is zero: no bracket ends there.
        let negated = real_roots(&[-2.0, 3.0, 0.0, -1.0]);
        assert!(negated.len() == 1 && (negated[0] + 2.0).abs() <= 1e-15, "{negated:?}");
        let line = real_roots(&[-3.0, 2.0]);
        assert!(line.len() == 1 && (line[0] - 1.5).abs() <= 1e-15, "{line:?}");
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
