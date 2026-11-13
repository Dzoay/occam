//! Source: piegl-tiller-1997
//! The B-spline basis (Piegl and Tiller, chapter 2): the span holding a parameter (A2.1), the functions nonzero there by
//! the triangular recurrence (A2.2), and their derivatives (eq. 2.9), each order from the functions one degree lower, so
//! that only the rows of the degrees it needs are kept. Generic over [`Real`], so one formula runs as `f64` or in lanes,
//! where every knot is a stored number in every lane.

use occam_math::Real;

/// The span of `knots`, of degree `p` with `n` poles, holding `t`: the largest `s` from `p` to `n - 1` with
/// `knots[s] <= t`. Below the domain, and for NaN, it is `p`; at or above the domain's end, `n - 1`.
pub(crate) fn span(knots: &[f64], p: usize, n: usize, t: f64) -> usize {
    let inner = knots.get(p.saturating_add(1)..n).unwrap_or_default();
    p.saturating_add(inner.partition_point(|k| *k <= t))
}

/// The knots the functions nonzero on span `s` read: those below it, `knots[s - p + 1]` to `knots[s]`, and those above,
/// `knots[s + 1]` to `knots[s + p]`.
fn window(knots: &[f64], p: usize, s: usize) -> (&[f64], &[f64]) {
    let below = knots.get(s.saturating_add(1).saturating_sub(p)..=s).unwrap_or_default();
    let above = knots.get(s.saturating_add(1)..=s.saturating_add(p)).unwrap_or_default();
    (below, above)
}

/// Raises `row` from the `j` functions of degree `j - 1` nonzero on the span to the `j + 1` of degree `j` (A2.2).
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn raise<R: Real>(row: &mut [R], (below, above): (&[f64], &[f64]), j: usize, t: R) {
    let lows = below.get(below.len().saturating_sub(j)..).unwrap_or_default();
    let mut saved = R::splat(0.0);
    for ((n, &a), &b) in row.iter_mut().zip(lows).zip(above) {
        let temp = *n / R::splat(b - a);
        *n = saved + (R::splat(b) - t) * temp;
        saved = (t - R::splat(a)) * temp;
    }
    if let Some(last) = row.get_mut(j) {
        *last = saved;
    }
}

/// Writes into `into` the derivatives of the `q + 1` functions of degree `q` nonzero on the span, given in `from` the
/// derivatives one order lower of the `q` of degree `q - 1`: `N'(i, q) = q (N(i, q-1) / (u(i+q) - u(i)) - N(i+1, q-1) /
/// (u(i+q+1) - u(i+1)))`, differentiated as often as `from` was. No divisor is zero: each spans the span.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn differentiate<R: Real>(from: &[R], into: &mut [R], (below, above): (&[f64], &[f64]), q: usize) {
    let lows = below.get(below.len().saturating_sub(q)..).unwrap_or_default();
    let scale = R::splat(f64::from(u16::try_from(q).unwrap_or(u16::MAX)));
    let mut before = R::splat(0.0);
    for (((out, &value), &a), &b) in into.iter_mut().zip(from).zip(lows).zip(above) {
        let term = value / R::splat(b - a);
        *out = scale * (before - term);
        before = term;
    }
    if let Some(last) = into.get_mut(q) {
        *last = scale * before;
    }
}

/// Row `i - 1` of `rows`, rows of `width`, and row `i` to write.
fn pair<R>(rows: &mut [R], width: usize, i: usize) -> Option<(&[R], &mut [R])> {
    let (lower, upper) = rows.split_at_mut_checked(i.checked_mul(width)?)?;
    Some((lower.get(lower.len().checked_sub(width)?..)?, upper.get_mut(..width)?))
}

/// How many numbers [`basis`] works in for degree `p` to `order`.
pub(crate) const fn rows_len(p: usize, order: usize) -> usize {
    let n = if order < p { order } else { p };
    n.saturating_add(1).saturating_mul(p.saturating_add(1))
}

/// Writes into `ders` the derivatives of order 0 to `order` at `t` of the `p + 1` functions of degree `p` nonzero on
/// span `s`, a row of `p + 1` for each order, lowest first, and zero rows above order `p`. It keeps in `rows`, which
/// holds [`rows_len`] numbers, the functions of each degree from `p - order` to `p`, and differentiates them in place,
/// highest degree first, so that each order reads the one below it.
pub(crate) fn basis<R: Real>(
    knots: &[f64],
    (p, s): (usize, usize),
    t: R,
    order: usize,
    rows: &mut [R],
    ders: &mut [R],
) {
    let (width, n) = (p.saturating_add(1), order.min(p));
    let first = p.saturating_sub(n);
    let window = window(knots, p, s);
    if let Some(start) = rows.get_mut(..width) {
        start.fill(R::splat(0.0));
        if let Some(one) = start.first_mut() {
            *one = R::splat(1.0);
        }
        for j in 1..=first {
            raise(start, window, j, t);
        }
    }
    for i in 1..=n {
        if let Some((from, into)) = pair(rows, width, i) {
            into.copy_from_slice(from);
            raise(into, window, first.saturating_add(i), t);
        }
    }
    let top = n.saturating_mul(width);
    let mut out = ders.chunks_exact_mut(width);
    let mut emit = |rows: &[R]| {
        if let (Some(into), Some(row)) = (out.next(), rows.get(top..top.saturating_add(width))) {
            into.copy_from_slice(row);
        }
    };
    emit(rows);
    for k in 1..=n {
        for i in (k..=n).rev() {
            if let Some((from, into)) = pair(rows, width, i) {
                differentiate(from, into, window, first.saturating_add(i));
            }
        }
        emit(rows);
    }
    for rest in out.take(order.saturating_sub(n)) {
        rest.fill(R::splat(0.0));
    }
}

/// The binomial coefficient `n` choose `k`, exact while it is below 2^53.
pub(crate) fn binomial(n: usize, k: usize) -> f64 {
    let as_f64 = |x: usize| f64::from(u32::try_from(x).unwrap_or(u32::MAX));
    (1..=k.min(n)).fold(1.0, |c, m| c * as_f64(n.saturating_sub(k).saturating_add(m)) / as_f64(m))
}

#[cfg(test)]
mod tests {
    use occam_math::Lanes;

    use super::*;

    /// A cubic with a double knot inside, and an unclamped quadratic.
    const CUBIC: [f64; 11] = [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0];

    #[expect(clippy::arithmetic_side_effects, reason = "small indices in a test")]
    fn all(knots: &[f64], (p, s): (usize, usize), t: f64, order: usize) -> Vec<Vec<f64>> {
        let (mut rows, mut ders) = (vec![0.0; rows_len(p, order)], vec![9.0; (order + 1) * (p + 1)]);
        basis(knots, (p, s), t, order, &mut rows, &mut ders);
        ders.chunks(p + 1).map(<[f64]>::to_vec).collect()
    }

    #[test]
    fn the_span_is_the_last_knot_at_or_below_and_stays_inside_the_domain() {
        let spans: Vec<usize> = [-1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, f64::NAN]
            .into_iter()
            .map(|t| span(&CUBIC, 3, 7, t))
            .collect();
        assert_eq!(spans, [3, 3, 3, 4, 4, 6, 6, 6, 6, 3]);
    }

    #[test]
    fn the_functions_sum_to_one_and_match_the_closed_form() {
        for t in [0.0, 0.3, 1.0, 1.7, 2.0, 2.4, 3.0] {
            let s = span(&CUBIC, 3, 7, t);
            let ders = all(&CUBIC, (3, s), t, 4);
            assert!((ders[0].iter().sum::<f64>() - 1.0).abs() < 1e-15, "{t}: {ders:?}");
            assert!(ders[1].iter().sum::<f64>().abs() < 1e-14 && ders[2].iter().sum::<f64>().abs() < 1e-13);
            assert_eq!(ders[4], [0.0; 4], "a cubic has no fourth derivative");
        }
        // On the first span, from 0 to 1 with knots 0, 0, 0, 0, 1, 2, the first function is (1 - t)^3.
        let t = 0.3;
        let ders = all(&CUBIC, (3, 3), t, 3);
        let want = [(1.0 - t) * (1.0 - t) * (1.0 - t), -3.0 * (1.0 - t) * (1.0 - t), 6.0 * (1.0 - t), -6.0];
        for (k, w) in want.into_iter().enumerate() {
            assert!((ders[k][0] - w).abs() < 1e-14, "order {k}: {} against {w}", ders[k][0]);
        }
    }

    #[test]
    fn derivatives_agree_with_differences_of_the_order_below() {
        let quadratic = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        for (knots, p, n, t) in [(&CUBIC[..], 3, 7, 1.4), (&CUBIC[..], 3, 7, 2.6), (&quadratic[..], 2, 4, 2.5)] {
            let s = span(knots, p, n, t);
            let h = 1e-6;
            let (here, up, down) =
                (all(knots, (p, s), t, 3), all(knots, (p, s), t + h, 3), all(knots, (p, s), t - h, 3));
            for k in 1..=3 {
                for r in 0..=p {
                    let central = (up[k - 1][r] - down[k - 1][r]) / (2.0 * h);
                    assert!((here[k][r] - central).abs() < 1e-6 * (1.0 + central.abs()), "{p} {t} {k} {r}");
                }
            }
        }
    }

    #[test]
    fn lanes_give_the_bits_of_single_numbers() {
        let ts = [0.1, 0.35, 0.7, 0.95];
        let mut rows = [Lanes::<4>::splat(0.0); 16];
        let mut ders = [Lanes::<4>::splat(0.0); 12];
        basis(&CUBIC, (3, 3), Lanes::new(ts), 2, &mut rows, &mut ders);
        for (lane, t) in ts.into_iter().enumerate() {
            let one: Vec<f64> = all(&CUBIC, (3, 3), t, 2).concat();
            let got: Vec<f64> = ders.iter().map(|d| d.to_array()[lane]).collect();
            assert_eq!(
                got.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                one.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn binomials_are_exact_where_they_can_be() {
        assert_eq!(
            [binomial(0, 0), binomial(4, 2), binomial(5, 0), binomial(5, 5), binomial(7, 3)],
            [1.0, 6.0, 1.0, 1.0, 35.0]
        );
        assert_eq!(binomial(40, 20), 137_846_528_820.0);
    }
}
