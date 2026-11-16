//! Source: original
//! Truncated Taylor series in one parameter, the numbers `f_k = f⁽ᵏ⁾(t) / k!` from `k = 0`, for derivatives of any order
//! of a closed form built from a curve's: sums are coefficient by coefficient, a product is Leibniz's rule, and a
//! quotient and a square root are the same rule solved for the next coefficient. An angle's series follows from its
//! rate, `θ' = (x y' − y x') / (x² + y²)`, given its value. Every series is as long as the shortest it is made from.

/// The series `a + b`.
pub(crate) fn plus(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

/// The series `a` times the number `by`, with `shift` added to its value.
pub(crate) fn scaled(a: &[f64], by: f64, shift: f64) -> Vec<f64> {
    a.iter().enumerate().map(|(k, x)| x * by + if k == 0 { shift } else { 0.0 }).collect()
}

/// The series `a b`: `c_k = Σ a_j b_{k−j}`.
pub(crate) fn product(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len().min(b.len());
    (0..n)
        .map(|k| {
            let terms = k.saturating_add(1);
            a.iter().take(terms).zip(b.iter().take(terms).rev()).map(|(x, y)| x * y).sum()
        })
        .collect()
}

/// The series `a / b`: `c_k = (a_k − Σ_{j ≥ 1} b_j c_{k−j}) / b_0`.
pub(crate) fn quotient(a: &[f64], b: &[f64]) -> Vec<f64> {
    let first = b.first().copied().unwrap_or(f64::NAN);
    let mut c: Vec<f64> = Vec::with_capacity(a.len());
    for (k, ak) in a.iter().enumerate().take(b.len()) {
        let carried: f64 = b.iter().skip(1).take(k).zip(c.iter().rev()).map(|(x, y)| x * y).sum();
        c.push((ak - carried) / first);
    }
    c
}

/// The series `√a`: `s_k = (a_k − Σ_{0 < j < k} s_j s_{k−j}) / 2 s_0`.
pub(crate) fn root(a: &[f64]) -> Vec<f64> {
    let mut s: Vec<f64> = Vec::with_capacity(a.len());
    for ak in a {
        let next = match s.first().copied() {
            None => ak.sqrt(),
            Some(first) => {
                let inner = s.len().saturating_sub(1);
                let carried: f64 = s.iter().skip(1).zip(s.iter().rev()).take(inner).map(|(x, y)| x * y).sum();
                (ak - carried) / (2.0 * first)
            }
        };
        s.push(next);
    }
    s
}

/// The series of `a`'s rate, one shorter: `(k + 1) a_{k+1}`.
fn rate(a: &[f64]) -> Vec<f64> {
    a.iter().zip(1_u32..).skip(1).map(|(x, k)| x * f64::from(k.saturating_sub(1))).collect()
}

/// The series of the angle of `(x, y)` whose value is `at`.
pub(crate) fn angle(at: f64, x: &[f64], y: &[f64]) -> Vec<f64> {
    let turning = plus(&product(x, &rate(y)), &scaled(&product(y, &rate(x)), -1.0, 0.0));
    let square = plus(&product(x, x), &product(y, y));
    let speed = quotient(&turning, &square);
    std::iter::once(at).chain(speed.iter().zip(1_u32..).map(|(s, k)| s / f64::from(k))).collect()
}

/// The derivatives of orders 1 to the series' last from its coefficients: `k! f_k`.
pub(crate) fn derivatives(a: &[f64]) -> Vec<f64> {
    let mut factorial = 1.0;
    a.iter()
        .zip(1_u32..)
        .skip(1)
        .map(|(x, k)| {
            factorial *= f64::from(k.saturating_sub(1));
            x * factorial
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The series of `exp(s t)` about 0, to order `n`.
    fn exponential(s: f64, n: u32) -> Vec<f64> {
        let mut term = 1.0;
        (0..=n)
            .map(|k| {
                let out = term;
                term *= s / f64::from(k.saturating_add(1));
                out
            })
            .collect()
    }

    fn agree(got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len());
        for (k, (g, w)) in got.iter().zip(want).enumerate() {
            assert!((g - w).abs() <= 1e-13 * (1.0 + w.abs()), "coefficient {k}: {g} against {w}");
        }
    }

    #[test]
    fn products_quotients_and_roots_follow_the_exponential() {
        let (a, b) = (exponential(0.7, 6), exponential(-0.3, 6));
        agree(&product(&a, &b), &exponential(0.4, 6));
        agree(&quotient(&a, &b), &exponential(1.0, 6));
        agree(&root(&a), &exponential(0.35, 6));
        agree(&scaled(&a, 2.0, 1.0), &[3.0, 1.4, 0.49, 0.343 / 3.0, 0.2401 / 12.0, 0.168_07 / 60.0, 0.117_649 / 360.0]);
    }

    #[test]
    fn an_angle_turning_steadily_on_a_circle_has_that_rate_and_no_other() {
        // (cos 2t, sin 2t) about t = 0.3: the angle is 2t.
        let (sin, cos) = occam_math::sin_cos(0.6);
        let (mut x, mut y) = (vec![], vec![]);
        let mut scale = 1.0;
        for k in 0..6_u32 {
            // The k-th derivative of cos 2t is 2^k cos(2t + kπ/2), and of sin 2t is 2^k sin(2t + kπ/2).
            let (c, s) = match k % 4 {
                0 => (cos, sin),
                1 => (-sin, cos),
                2 => (-cos, -sin),
                _ => (sin, -cos),
            };
            x.push(c * scale);
            y.push(s * scale);
            scale *= 2.0 / f64::from(k.saturating_add(1));
        }
        let theta = angle(0.6, &x, &y);
        agree(&theta, &[0.6, 2.0, 0.0, 0.0, 0.0, 0.0]);
        agree(&derivatives(&theta), &[2.0, 0.0, 0.0, 0.0, 0.0]);
    }
}
