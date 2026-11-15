//! Source: original
//! The workloads for browser timing. Bicubic Bézier surface points by de Casteljau's algorithm, two lanes at a time:
//! `gates perf` builds it with and without simd128 and compares the two in one run (L30). And each lane kernel of
//! `occam_math::workloads`, and the spline kernel, at width 1 and at its width, which the simd128 build must show clearly
//! faster (L29). And each construction operation `gates perf` counts natively, run through a session as the editing
//! worker runs it, which the simd128 build must answer with the same bits (L23).
#![allow(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
#![allow(missing_docs, reason = "an example")]

use std::cell::RefCell;
use std::hint::black_box;

use occam::operation_workloads::{MEASURES, PRIMITIVES, SWEEPS, TRANSFORMS};
use occam::{Encoding, Jets, Lanes, Real, Session, SplineSurface, ran_json, spline_workloads, width, workloads};
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

/// Writes each row's points to a buffer, two at a time, as a tessellator does. When this workload summed the lanes straight
/// into one number, the simd128 build held no vector arithmetic at all, so check the gate's ratio after changing its shape.
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

/// A lane kernel's inputs: how many numbers make one item, their scale, and a seed.
fn items(name: &str) -> Vec<f64> {
    match name {
        "masks" => workloads::inputs(3, 4.0, 1),
        "filter" => workloads::inputs(6, 2.0, 2),
        "sines" => workloads::inputs(1, 4.0, 3),
        "angles" => workloads::inputs(2, 2.0, 4),
        _ => Vec::new(),
    }
}

/// The width a lane kernel runs at on wasm32, or 0 for no such kernel.
#[wasm_bindgen]
#[must_use]
pub fn kernel_width(name: &str) -> u32 {
    let found = match name {
        "masks" => width::MASK,
        "filter" => width::FILTER,
        "sines" | "angles" => width::ANALYTIC,
        "splines" => width::SPLINE,
        _ => 0,
    };
    u32::try_from(found).unwrap_or(0)
}

/// The spline kernel `repeats` times over a grid of 64 by 64 points, at its width or at width 1: the sum of every number
/// it wrote.
fn splines(lanes: bool, repeats: u32) -> f64 {
    let Ok(surface) = SplineSurface::new(&spline_workloads::spline()) else { return f64::NAN };
    let uv = spline_workloads::grid(64);
    let mut out = Jets::default();
    let mut total = 0.0;
    for _ in 0..repeats {
        if lanes {
            spline_workloads::splines::<{ width::SPLINE }, _>(black_box(&surface), black_box(&uv), &mut out);
        } else {
            spline_workloads::splines::<1, _>(black_box(&surface), black_box(&uv), &mut out);
        }
        total += spline_workloads::total(&out);
    }
    total
}

/// Runs a lane kernel `repeats` times over its inputs, at its width or at width 1, and gives the sum of every result.
/// The same bits at either width (L23).
#[wasm_bindgen]
#[must_use]
pub fn kernel(name: &str, lanes: bool, repeats: u32) -> f64 {
    if name == "splines" {
        return splines(lanes, repeats);
    }
    let items = items(name);
    let run = |items: &[f64]| -> Vec<f64> {
        match (name, lanes) {
            ("masks", false) => workloads::masks::<f64>(items),
            ("masks", true) => workloads::masks::<Lanes<{ width::MASK }>>(items),
            ("filter", false) => workloads::filter::<f64>(items),
            ("filter", true) => workloads::filter::<Lanes<{ width::FILTER }>>(items),
            ("sines", false) => workloads::sines::<f64>(items),
            ("sines", true) => workloads::sines::<Lanes<{ width::ANALYTIC }>>(items),
            ("angles", false) => workloads::angles::<f64>(items),
            ("angles", true) => workloads::angles::<Lanes<{ width::ANALYTIC }>>(items),
            _ => Vec::new(),
        }
    };
    let mut total = 0.0;
    for _ in 0..repeats {
        total += run(black_box(&items)).iter().sum::<f64>();
    }
    total
}

thread_local! {
    /// The primitives, made once for the measures: setup, which the timing leaves out after the first round.
    static PRIMITIVES_MADE: RefCell<Option<Session>> = const { RefCell::new(None) };
}

/// FNV-1a, 64 bits, of `text`: enough to tell two builds' answers apart without passing them whole.
fn digest(text: &str) -> String {
    let hash = text
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

/// The session a measure runs in, the primitives made in it.
fn primitives() -> Session {
    PRIMITIVES_MADE.with(|made| {
        made.borrow_mut()
            .get_or_insert_with(|| {
                let mut session = Session::new(Encoding::Binary);
                let _ = session.run(PRIMITIVES);
                session
            })
            .clone()
    })
}

/// Runs the construction operation `name`, as `gates perf` names its class, once, and gives a digest of the run's JSON
/// form, or `failed` if the run did not go through, or `unknown` for no such operation.
#[wasm_bindgen]
#[must_use]
pub fn operation(name: &str) -> String {
    let (mut session, request) = match name {
        "construct_primitives" => (Session::new(Encoding::Binary), PRIMITIVES),
        "construct_sweeps" => (Session::new(Encoding::Binary), SWEEPS),
        "construct_transforms" => (Session::new(Encoding::Binary), TRANSFORMS),
        "measure_primitives" => (primitives(), MEASURES),
        _ => return "unknown".to_string(),
    };
    let (ran, _) = session.run(black_box(request));
    if ran.failed.is_some() { "failed".to_string() } else { digest(&ran_json(&ran)) }
}
