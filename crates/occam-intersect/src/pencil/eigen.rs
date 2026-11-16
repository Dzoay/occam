//! Source: original
//! The eigenvalues and unit eigenvectors of a small symmetric matrix, by Jacobi's rotations: each rotation zeroes one
//! entry off the diagonal, and sweeps of them drive the rest to zero. It needs only `+ − × ÷` and the square root.

/// `a[i][j]`, or 0 outside it.
fn entry<const N: usize>(a: &[[f64; N]; N], i: usize, j: usize) -> f64 {
    a.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0.0)
}

fn set<const N: usize>(a: &mut [[f64; N]; N], i: usize, j: usize, value: f64) {
    if let Some(slot) = a.get_mut(i).and_then(|row| row.get_mut(j)) {
        *slot = value;
    }
}

/// One rotation of the symmetric `matrix` in the plane of `p` and `q`, carried into the columns of `vectors`.
fn rotate<const N: usize>(matrix: &mut [[f64; N]; N], vectors: &mut [[f64; N]; N], (p, q): (usize, usize)) {
    let apq = entry(matrix, p, q);
    if apq == 0.0 {
        return;
    }
    let theta = (entry(matrix, q, q) - entry(matrix, p, p)) / (2.0 * apq);
    let tangent = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
    let cosine = 1.0 / (tangent * tangent + 1.0).sqrt();
    let sine = tangent * cosine;
    let turn = |m: &mut [[f64; N]; N], (i, j): (usize, usize), (k, l): (usize, usize)| {
        let (first, second) = (entry(m, i, j), entry(m, k, l));
        set(m, i, j, cosine * first - sine * second);
        set(m, k, l, sine * first + cosine * second);
    };
    for row in 0..N {
        turn(matrix, (row, p), (row, q));
    }
    for column in 0..N {
        turn(matrix, (p, column), (q, column));
    }
    for row in 0..N {
        turn(vectors, (row, p), (row, q));
    }
}

/// The eigenvalues of the symmetric `m`, smallest in size first, and their unit eigenvectors, in the same order.
pub(super) fn eigen<const N: usize>(m: [[f64; N]; N]) -> ([f64; N], [[f64; N]; N]) {
    let mut a = m;
    let mut v = [[0.0; N]; N];
    for i in 0..N {
        set(&mut v, i, i, 1.0);
    }
    let pairs: Vec<(usize, usize)> = (0..N).flat_map(|p| (p.saturating_add(1)..N).map(move |q| (p, q))).collect();
    for _ in 0..40 {
        if pairs.iter().all(|&(p, q)| entry(&a, p, q) == 0.0) {
            break;
        }
        for &plane in &pairs {
            rotate(&mut a, &mut v, plane);
        }
    }
    let mut order: Vec<usize> = (0..N).collect();
    order.sort_by(|&i, &j| entry(&a, i, i).abs().total_cmp(&entry(&a, j, j).abs()));
    let values =
        std::array::from_fn(|k| entry(&a, order.get(k).copied().unwrap_or(k), order.get(k).copied().unwrap_or(k)));
    let vectors = std::array::from_fn(|k| {
        let j = order.get(k).copied().unwrap_or(k);
        std::array::from_fn(|i| entry(&v, i, j))
    });
    (values, vectors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symmetric_matrix_is_its_eigenvectors_scaled_by_its_eigenvalues() {
        let m = [[2.0, -1.0, 0.5, 0.0], [-1.0, 0.3, 0.2, 1.0], [0.5, 0.2, -1.5, 0.4], [0.0, 1.0, 0.4, 0.01]];
        let (values, vectors) = eigen(m);
        for (value, v) in values.iter().zip(&vectors) {
            for (row, mi) in m.iter().enumerate() {
                let mv: f64 = mi.iter().zip(v).map(|(a, b)| a * b).sum();
                assert!((mv - value * v[row]).abs() < 1e-13, "{value}: {mv} against {}", value * v[row]);
            }
        }
        assert!(values.windows(2).all(|w| w[0].abs() <= w[1].abs()), "{values:?}");
    }
}
