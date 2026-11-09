//! Source: original
//! The workload for browser timing: bicubic Bézier surface points by de Casteljau's algorithm, two lanes at a time. `gates
//! perf` builds it with and without simd128 and compares the two in one run (L30).
#![allow(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
#![allow(missing_docs, reason = "an example")]

use std::hint::black_box;

use occam::{Lanes, Real};
use wasm_bindgen::prelude::wasm_bindgen;

/// A point on the cubic with these control points, at `at`.
fn cubic<R: Real>([p0, p1, p2, p3]: [R; 4], at: R) -> R {
    let rest = R::splat(1.0) - at;
    let (first, second, third) = (p0 * rest + p1 * at, p1 * rest + p2 * at, p2 * rest + p3 * at);
    let (left, right) = (first * rest + second * at, second * rest + third * at);
    left * rest + right * at
}

/// The control net, through `black_box` so that the compiler has to compute the surface and cannot fold it away.
fn control_net() -> [[f64; 4]; 4] {
    black_box([[0.0, 1.5, 3.0, 2.0], [1.0, 2.5, 0.5, 1.0], [2.0, 0.5, 1.0, 3.5], [1.5, 3.0, 2.5, 0.5]])
}

/// A point on the bicubic surface over a control net.
fn surface<R: Real>([row0, row1, row2, row3]: [[f64; 4]; 4], u: R, v: R) -> R {
    let along = |row: [f64; 4]| cubic(row.map(R::splat), u);
    cubic([along(row0), along(row1), along(row2), along(row3)], v)
}

/// Writes each row's points to a buffer, two at a time, as a tessellator does. The vectoriser starts from those stores: when
/// this workload summed the lanes straight into one number, the simd128 build held no vector arithmetic at all.
fn sweep(grid: u32) -> f64 {
    let net = control_net();
    let step = 1.0 / f64::from(grid.max(1));
    let mut row = vec![0.0; usize::try_from(grid).unwrap_or(0)];
    let mut total = 0.0;
    for j in 0..grid {
        let v = Lanes::splat(f64::from(j) * step);
        let mut i = 0_u32;
        for pair in row.as_chunks_mut::<2>().0 {
            let u = Lanes::<2>::from_fn(|| {
                let at = f64::from(i) * step;
                i = i.saturating_add(1);
                at
            });
            *pair = surface(net, u, v).to_array();
        }
        total += row.iter().sum::<f64>();
    }
    total
}

/// The sum of every surface point on a `grid` by `grid` lattice. The same bits with or without simd128 (L23).
#[wasm_bindgen]
#[must_use]
pub fn run(grid: u32) -> f64 {
    sweep(grid)
}
