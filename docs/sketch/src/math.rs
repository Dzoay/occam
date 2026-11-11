//! `occam-math`: what it gains. Everything that computes is here (the maths choke point decision), with the two approved
//! crates that can touch bits: `fearless_simd`, named only by `Level` and `dispatch`, and `rayon`, named only by `Exec`.

use std::num::NonZeroUsize;
use std::ops::Range;
#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

use occam_math::Real;
use occam_report::Report;

/// The result of a lane comparison: `bool`, or one `bool` per lane.
pub trait Mask: Copy + Send + Sync + 'static {
    fn any(self) -> bool;
    fn all(self) -> bool;
}

impl Mask for bool {
    fn any(self) -> bool {
        self
    }

    fn all(self) -> bool {
        self
    }
}

impl<const W: usize> Mask for [bool; W] {
    fn any(self) -> bool {
        self.iter().any(|&lane| lane)
    }

    fn all(self) -> bool {
        self.iter().all(|&lane| lane)
    }
}

/// CHANGED: what `Real` gains, shown as an extension; in the crate these join `Real` itself. S4 and P5 had to build
/// comparison through `to_array`, and S5 says to own the two functions analytic evaluation runs in bulk. `Lanes::from_fn`
/// also becomes a plain loop (P4: `array::from_fn` falling out of line cost a rational kernel 2.3 times).
pub trait RealNext: Real {
    type Mask: Mask;

    /// Exact; false in a lane where either side is NaN.
    fn lt(self, rhs: Self) -> Self::Mask;
    fn le(self, rhs: Self) -> Self::Mask;
    /// The way to a minimum or a maximum: `f64::min` stays banned, since which zero it returns is unspecified.
    fn select(mask: Self::Mask, yes: Self, no: Self) -> Self;
    /// Owned (S5, O14): the same bits on every target, lane width and build; within about 0.75 ulp.
    fn sin_cos(self) -> (Self, Self);
    /// Owned (S5, O14): within about 1.5 ulp.
    fn atan2(self, x: Self) -> Self;
}

/// The widest instruction level this CPU has, found once.
#[derive(Clone, Copy, Debug)]
pub struct Level(fearless_simd::Level);

impl Level {
    #[must_use]
    pub fn detect() -> Self {
        Self(fearless_simd::Level::new())
    }

    /// The level of the plain build, so `gates determinism` can compare every level's bits with it.
    #[must_use]
    pub const fn baseline() -> Self {
        Self(fearless_simd::Level::baseline())
    }
}

/// A lane kernel. `run` and everything it calls are `#[inline(always)]`, or a leaf is compiled without the level's
/// features (P4). A trait rather than a closure, because a closure cannot carry that attribute on stable Rust.
pub trait LaneKernel {
    type Output;

    fn run(self) -> Self::Output;
}

/// Runs `kernel` compiled for `level`, with plain `Lanes<W>` arithmetic: the same bits at every level, since nothing
/// fuses. `DETERMINISM.md` rule 5, no `target-cpu`, stands (P4).
#[inline(never)]
pub fn dispatch<K: LaneKernel>(level: Level, kernel: K) -> K::Output {
    fearless_simd::dispatch!(level.0, _simd => kernel.run())
}

/// A midpoint and a radius, each operation widening the radius by a bound on its own rounding, with `+ - * /` and
/// `sqrt` only, so no rounding mode changes (S2; Rump 1999).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval<R: Real = f64> {
    mid: R,
    rad: R,
}

impl<R: Real> Interval<R> {
    pub fn exact(x: R) -> Self {
        Self { mid: x, rad: R::splat(0.0) }
    }

    /// The certified error radius of a computed value (S2 item 3). It is not a tolerance and obeys no growth rule.
    pub const fn radius(self) -> R {
        self.rad
    }
}

/// Where a quantity lies against the band `[-eps, eps]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Below,
    Within,
    Above,
}

/// A decision on a computed value. Its enclosure can straddle an edge of the band: then no decision is made, "cannot
/// decide at this resolution" (S2 item 3, O11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Decided(Side),
    Undecided,
}

/// What a predicate is written over, once: `Interval` for the filter, `Expansion` for the exact stage (S2).
pub trait Ring: Clone + Sized {
    fn of(x: f64) -> Self;
    #[must_use]
    fn add(&self, rhs: &Self) -> Self;
    #[must_use]
    fn sub(&self, rhs: &Self) -> Self;
    #[must_use]
    fn mul(&self, rhs: &Self) -> Self;
}

/// An exact sum of non-overlapping doubles (Shewchuk 1997): about 3.3 µs a decision (P6). No exact towers by default
/// (S2: their work grows about five times a step).
#[derive(Clone, Debug, PartialEq)]
pub struct Expansion(Vec<f64>);

impl Ring for Interval<f64> {
    fn of(x: f64) -> Self {
        Self::exact(x)
    }

    fn add(&self, rhs: &Self) -> Self {
        unimplemented!()
    }

    fn sub(&self, rhs: &Self) -> Self {
        unimplemented!()
    }

    fn mul(&self, rhs: &Self) -> Self {
        unimplemented!()
    }
}

impl Ring for Expansion {
    fn of(x: f64) -> Self {
        Self(vec![x])
    }

    fn add(&self, rhs: &Self) -> Self {
        unimplemented!()
    }

    fn sub(&self, rhs: &Self) -> Self {
        unimplemented!()
    }

    fn mul(&self, rhs: &Self) -> Self {
        unimplemented!()
    }
}

/// A polynomial in `N` inputs, written once over `Ring`. S2's D1 to D5 are predicates in `occam-geom` and
/// `occam-intersect`.
pub trait Predicate<const N: usize>: Sync {
    fn value<T: Ring>(&self, inputs: &[T; N]) -> T;
}

/// Decisions made, and how many reached the exact stage: the rate P6 says to record, since each 1% adds about 33 ns a
/// decision on one thread. Every operation reports it as a `measure.decisions` note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub decided: u64,
    pub exact: u64,
    pub undecided: u64,
}

/// Decisions on stored numbers (S2 item 2): the interval filter in `Lanes<4>` (P6), the exact stage inline for the lanes
/// it cannot certify. Never undecided.
pub fn decide_stored<const N: usize, P: Predicate<N>>(
    predicate: &P,
    inputs: &[[f64; N]],
    eps: f64,
    out: &mut Vec<Side>,
) -> Tally {
    unimplemented!()
}

/// Decisions on computed values, which carry their certified radius: these may be `Undecided` (S2 item 3).
pub fn decide_computed<const N: usize, P: Predicate<N>>(
    predicate: &P,
    inputs: &[[Interval; N]],
    eps: f64,
    out: &mut Vec<Verdict>,
) -> Tally {
    unimplemented!()
}

/// How work is split into tasks. It changes speed, never bits (P11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grain {
    /// Left to the runner, for cheap items such as box tests: under `rayon`, 3.94 times at 100 ns an item (P11).
    Auto,
    /// One item a task, for items of 10 µs and more: a face, a pair, a region (P2, P9, P11).
    Each,
}

/// The execution layer: P11's ordered map, with `rayon` behind it (decision 2026-11-10). A concrete type, so no kernel
/// function is generic over its runner, and a swap to another runner stays inside this type.
#[derive(Clone, Debug)]
pub struct Exec {
    runner: Runner,
}

#[derive(Clone, Debug)]
enum Runner {
    /// The one-thread baseline, and the runner inside each browser worker (P10).
    Serial,
    #[cfg(not(target_family = "wasm"))]
    Pool(Arc<rayon::ThreadPool>),
}

impl Exec {
    #[must_use]
    pub const fn serial() -> Self {
        Self { runner: Runner::Serial }
    }

    /// # Errors
    ///
    /// `exec.threads-unavailable` if the pool cannot start. On wasm this is `serial()`.
    pub fn threads(count: NonZeroUsize) -> Result<Self, Report> {
        unimplemented!()
    }

    /// `f` over `0..n`, the results in index order whatever order they finished in. No reductions, so nothing can depend
    /// on a split tree (P11: `rayon`'s float `sum` gave 7 bit patterns in 8 runs). Closures are `'static`, which keeps a
    /// swap to a small pool possible.
    pub fn map<R, F>(&self, n: usize, grain: Grain, f: F) -> Vec<R>
    where
        R: Send + 'static,
        F: Fn(usize) -> R + Send + Sync + 'static,
    {
        match &self.runner {
            Runner::Serial => (0..n).map(f).collect(),
            #[cfg(not(target_family = "wasm"))]
            Runner::Pool(pool) => {
                use rayon::prelude::*;
                let most = match grain {
                    Grain::Auto => usize::MAX,
                    Grain::Each => 1,
                };
                pool.install(|| (0..n).into_par_iter().with_max_len(most).map(f).collect())
            }
        }
    }

    /// A batch of `len` items in chunks of `chunk`, each chunk a task claimed dynamically and a lane group, the results
    /// concatenated in item order. Chunk boundaries depend on `chunk`, never on the thread count. P6's predicate batches
    /// run here at 256 to 1,024 items a chunk.
    pub fn map_chunks<R, F>(&self, len: usize, chunk: NonZeroUsize, f: F) -> Vec<R>
    where
        R: Send + 'static,
        F: Fn(Range<usize>) -> Vec<R> + Send + Sync + 'static,
    {
        let size = chunk.get();
        let parts = self.map(len.div_ceil(size), Grain::Each, move |index| {
            let start = index.saturating_mul(size);
            f(start..start.saturating_add(size).min(len))
        });
        parts.into_iter().flatten().collect()
    }
}
