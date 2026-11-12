//! Source: piegl-tiller-1997
//! Knot insertion (Piegl and Tiller, section 5.2, after Boehm): a knot added to a B-spline without changing its shape,
//! and so the part of a B-spline between two parameters, each end inserted until it repeats as often as the degree.
//! Poles are homogeneous, so a rational B-spline is cut as its weighted form is.

use crate::basis::span;

/// A B-spline's shared knots and degree, and the poles of each of its rows: one row for a curve, and a row of constant
/// `v` or `u` at a time for a surface.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rows {
    /// The knots, repeats included.
    pub(crate) knots: Vec<f64>,
    /// The degree.
    pub(crate) degree: usize,
    /// The homogeneous poles of each row.
    pub(crate) poles: Vec<Vec<[f64; 4]>>,
}

/// `(1 - a) p + a q`, for homogeneous poles.
fn blend(p: [f64; 4], q: [f64; 4], a: f64) -> [f64; 4] {
    let [p0, p1, p2, p3] = p;
    let [q0, q1, q2, q3] = q;
    [(1.0 - a) * p0 + a * q0, (1.0 - a) * p1 + a * q1, (1.0 - a) * p2 + a * q2, (1.0 - a) * p3 + a * q3]
}

/// How many times `t` is a knot. Knots hold no -0, which [`canonical`] makes +0, so bits compare as numbers do.
fn multiplicity(knots: &[f64], t: f64) -> usize {
    knots.iter().filter(|k| k.to_bits() == t.to_bits()).count()
}

impl Rows {
    /// How many poles each row has.
    fn count(&self) -> usize {
        self.knots.len().saturating_sub(self.degree).saturating_sub(1)
    }

    /// `t`, inside the domain, inserted once: on span `k`, poles `k - p + 1` to `k` become blends of their neighbours,
    /// with `a(i) = (t - u(i)) / (u(i + p) - u(i))`, and those after move along one.
    fn insert(&mut self, t: f64) {
        let (p, n) = (self.degree, self.count());
        let k = span(&self.knots, p, n, t);
        let alphas: Vec<f64> = (k.saturating_sub(p).saturating_add(1)..=k)
            .map(|i| {
                let (low, high) = (self.knots.get(i).copied(), self.knots.get(i.saturating_add(p)).copied());
                low.zip(high).map_or(0.0, |(low, high)| (t - low) / (high - low))
            })
            .collect();
        let first = k.saturating_sub(p).saturating_add(1);
        for row in &mut self.poles {
            let mut next = Vec::with_capacity(n.saturating_add(1));
            next.extend(row.iter().take(first));
            let blended = (first..=k).zip(&alphas).filter_map(|(i, a)| {
                let (before, at) = (row.get(i.checked_sub(1)?)?, row.get(i)?);
                Some(blend(*before, *at, *a))
            });
            next.extend(blended);
            next.extend(row.iter().skip(k));
            *row = next;
        }
        self.knots.insert(k.saturating_add(1).min(self.knots.len()), t);
    }

    /// The part from `lo` to `hi`, which lie inside the domain with `lo` below `hi`: each end inserted until it repeats
    /// as often as the degree, then the poles from the one at `lo` to the one at `hi`, and the knots between, each end
    /// repeated once more than the degree.
    pub(crate) fn restricted(mut self, lo: f64, hi: f64) -> Self {
        let (p, lo, hi) = (self.degree, lo + 0.0, hi + 0.0);
        for t in [lo, hi] {
            for _ in multiplicity(&self.knots, t)..p {
                self.insert(t);
            }
        }
        let first = |t: f64| self.knots.iter().position(|k| k.to_bits() == t.to_bits());
        let start = first(lo).unwrap_or(0).saturating_add(multiplicity(&self.knots, lo)).saturating_sub(p);
        let end = first(hi).unwrap_or(self.knots.len());
        let between = self.knots.get(start.saturating_add(p)..end).unwrap_or_default();
        let ends = p.saturating_add(1);
        let knots = std::iter::repeat_n(lo, ends).chain(between.iter().copied()).chain(std::iter::repeat_n(hi, ends));
        let poles = |row: &Vec<[f64; 4]>| row.get(start.saturating_sub(1)..end).unwrap_or_default().to_vec();
        Self { knots: knots.collect(), degree: p, poles: self.poles.iter().map(poles).collect() }
    }
}

/// `knots` with -0 made +0, so that a knot's bits compare as its value does.
pub(crate) fn canonical(knots: &[f64]) -> Vec<f64> {
    knots.iter().map(|k| k + 0.0).collect()
}

/// The rows of a grid `columns` wide, read the other way: its columns.
pub(crate) fn transposed(rows: &[Vec<[f64; 4]>]) -> Vec<Vec<[f64; 4]>> {
    let columns = rows.first().map_or(0, Vec::len);
    (0..columns).map(|c| rows.iter().filter_map(|row| row.get(c).copied()).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The point of a curve of degree `p` at `t`, by de Boor's recurrence on the homogeneous poles, projected.
    #[expect(clippy::arithmetic_side_effects, clippy::many_single_char_names, reason = "the recurrence as written")]
    fn at(curve: &Rows, t: f64) -> [f64; 3] {
        let (p, n) = (curve.degree, curve.count());
        let k = span(&curve.knots, p, n, t);
        let mut d: Vec<[f64; 4]> = curve.poles[0][k - p..=k].to_vec();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = k - p + j;
                let a = (t - curve.knots[i]) / (curve.knots[i + p + 1 - r] - curve.knots[i]);
                d[j] = blend(d[j - 1], d[j], a);
            }
        }
        let [x, y, z, w] = d[p];
        [x / w, y / w, z / w]
    }

    fn cubic() -> Rows {
        let poles = [
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 2.0, 0.0, 2.0],
            [3.0, 1.0, 1.0, 1.0],
            [4.0, 3.0, 0.0, 0.5],
            [6.0, 0.0, 2.0, 1.0],
        ];
        Rows { knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 3.0, 3.0, 3.0], degree: 3, poles: vec![poles.to_vec()] }
    }

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-13)
    }

    #[test]
    fn an_inserted_knot_leaves_the_curve_where_it_was() {
        let mut more = cubic();
        more.insert(2.0);
        more.insert(2.0);
        more.insert(0.5);
        assert_eq!(more.knots, [0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0]);
        assert_eq!(more.poles[0].len(), 8);
        for t in [0.0, 0.25, 0.5, 1.0, 1.7, 2.0, 2.9, 3.0] {
            assert!(close(at(&more, t), at(&cubic(), t)), "{t}");
        }
    }

    #[test]
    fn a_part_keeps_the_points_and_parameters_it_had() {
        for (lo, hi) in [(0.5, 2.5), (1.0, 3.0), (0.0, 1.0), (0.0, 3.0), (2.0, 2.25)] {
            let part = cubic().restricted(lo, hi);
            assert_eq!((part.knots[0], part.knots[3], *part.knots.last().unwrap()), (lo, lo, hi), "{lo} {hi}");
            assert_eq!(part.knots.len(), part.poles[0].len().saturating_add(4));
            for f in [0.0, 0.1, 0.5, 0.77, 1.0] {
                let t = lo + (hi - lo) * f;
                assert!(close(at(&part, t), at(&cubic(), t)), "{lo} {hi} at {t}");
            }
        }
        assert_eq!(cubic().restricted(0.0, 3.0), cubic(), "the whole is itself");
    }

    #[test]
    fn an_unclamped_spline_is_clamped_at_its_ends() {
        let poles = vec![(0..5).map(|i| [f64::from(i), f64::from(i) * f64::from(i), 0.0, 1.0]).collect()];
        let open = Rows { knots: (0..8).map(f64::from).collect(), degree: 2, poles };
        let part = open.clone().restricted(2.0, 5.0);
        assert_eq!(part.knots[..3], [2.0; 3]);
        for t in [2.0, 3.3, 4.5, 5.0] {
            assert!(close(at(&part, t), at(&open, t)), "{t}");
        }
    }

    #[test]
    fn minus_zero_is_plus_zero_in_the_knots_and_at_the_ends_of_a_part() {
        assert_eq!(canonical(&[-0.0, 1.0]).iter().map(|k| k.to_bits()).collect::<Vec<_>>(), [0, 1f64.to_bits()]);
        let part = cubic().restricted(-0.0, 1.0);
        assert_eq!(part, cubic().restricted(0.0, 1.0), "the knot at -0 is the knot at 0");
        assert_eq!(part.knots[0].to_bits(), 0);
        // A part ending at -0, on a quadratic from -1 to 1 with a knot at 0.
        let poles = vec![(0..4).map(|i| [f64::from(i), 1.0, 0.0, 1.0]).collect()];
        let across = Rows { knots: vec![-1.0, -1.0, -1.0, 0.0, 1.0, 1.0, 1.0], degree: 2, poles };
        let bits = |rows: Rows| rows.knots.iter().map(|k| k.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(across.clone().restricted(-1.0, -0.0)), bits(across.restricted(-1.0, 0.0)));
    }

    #[test]
    fn a_grid_transposes_to_its_columns() {
        let rows = vec![vec![[1.0; 4], [2.0; 4], [3.0; 4]], vec![[4.0; 4], [5.0; 4], [6.0; 4]]];
        assert_eq!(transposed(&rows), [vec![[1.0; 4], [4.0; 4]], vec![[2.0; 4], [5.0; 4]], vec![[3.0; 4], [6.0; 4]]]);
        assert_eq!(transposed(&transposed(&rows)), rows);
    }
}
