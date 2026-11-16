//! Source: levin-1979
//! The smooth quartic (Levin 1979): `v = (−β ± √Δ)/α` on the rulings where `Δ ≥ 0`, a loop round the whole turn for
//! each sign where `Δ` has no root, and else a loop over each arc between two roots where it is positive, both signs
//! joined at the roots.

use occam_geom::Quartic;
use occam_math::sin_cos;

use super::Pair;
use super::quadric::{Along, Kind, Ruling};
use super::trig::Trig;

/// `c` with zeros after it, to five coefficients.
fn five(c: &Trig) -> [f64; 5] {
    std::array::from_fn(|i| c.0.get(i).copied().unwrap_or(0.0))
}

/// `Δ / F` for `F = cos(θ − m) − cos h`, which divides it: `E`, of degree one.
fn quotient(delta: &[f64; 5], (cos_h, cos_m, sin_m): (f64, f64, f64)) -> [f64; 3] {
    let [_, d1c, d1s, d2c, d2s] = *delta;
    let f0 = -cos_h;
    let e1 = 2.0 * (cos_m * d2c + sin_m * d2s);
    let e2 = 2.0 * (cos_m * d2s - sin_m * d2c);
    let e0 = cos_m * (d1c - f0 * e1) + sin_m * (d1s - f0 * e2);
    [e0, e1, e2]
}

/// A loop's numbers before the way its root is taken: the ruled surface's axes in the world, what it is, `α` and `β`.
fn head(pair: &Pair, ruling: &Ruling, along: &Along) -> Vec<f64> {
    let origin = pair.world(ruling.origin);
    let mut out: Vec<f64> = [origin, ruling.x, ruling.y, ruling.z].iter().flat_map(|v| [v.x, v.y, v.z]).collect();
    out.extend(match ruling.kind {
        Kind::Upright { radius } => [0.0, radius, 0.0],
        Kind::Slanted { sin, cos } => [1.0, sin, cos],
    });
    out.extend(five(&along.alpha));
    out.extend((0..3).map(|i| along.beta.0.get(i).copied().unwrap_or(0.0)));
    out
}

/// The loops of a smooth quartic on `ruling`: `None` where one cannot be made of its numbers.
pub(super) fn loops(pair: &Pair, ruling: &Ruling, along: &Along) -> Option<Vec<Quartic>> {
    let delta = five(&along.delta);
    let roots = along.delta.roots();
    let whole = std::f64::consts::TAU;
    let make = |root: &[f64]| -> Option<Quartic> {
        let numbers: Vec<f64> =
            head(pair, ruling, along).into_iter().chain(root.iter().copied()).chain([0.0, whole, 2.0]).collect();
        Quartic::restore(&numbers).ok()
    };
    if roots.is_empty() {
        if along.delta.at(0.0) <= 0.0 {
            return Some(Vec::new());
        }
        let signs = [1.0, -1.0].map(|sign| make(&[vec![0.0], delta.to_vec(), vec![sign]].concat()));
        return signs.into_iter().collect();
    }
    let ends = roots.iter().copied().zip(roots.iter().skip(1).copied().chain(roots.first().map(|r| r + whole)));
    ends.filter(|(from, to)| along.delta.at(0.5 * from + 0.5 * to) > 0.0)
        .map(|(from, to)| {
            let (middle, half) = (0.5 * from + 0.5 * to, 0.5 * (to - from));
            let ((sin_m, cos_m), (_, cos_h), (k, _)) = (sin_cos(middle), sin_cos(half), sin_cos(0.5 * half));
            let e = quotient(&delta, (cos_h, cos_m, sin_m));
            make(&[1.0, cos_m, sin_m, k, e[0], e[1], e[2]])
        })
        .collect()
}
