//! Source: original
//! The one maths implementation (L23): every number the kernel computes goes through this crate.
//!
//! [`Real`] is the number type maths is written over, once, and is run as `f64` or as [`Lanes`] of `f64`. Same inputs give
//! identical bits on every target, at every thread count, scalar and in lanes. `docs/DETERMINISM.md` is the contract, and
//! `gates determinism` holds it.

#![expect(
    clippy::inline_always,
    reason = "a kernel compiles at a dispatched instruction level only through what is inlined into it (P4)"
)]

mod dispatch;
mod exact;
mod exec;
mod exp;
mod expansion;
mod hash;
mod hyperbolic;
mod interval;
mod lanes;
mod mask;
mod predicate;
mod real;
mod splitmix;
pub mod stream;
#[cfg(test)]
mod testing;
mod trig;
pub mod width;
pub mod workloads;

pub use dispatch::{LaneKernel, Level, dispatch};
pub use exact::{fast_two_sum, split, two_product, two_sum};
pub use exec::{Exec, Grain};
pub use exp::{exp, expm1};
pub use expansion::Expansion;
pub use hash::{CANONICAL_NAN, Digest, canonical_bits};
pub use hyperbolic::{cosh, sinh, sinh_cosh};
pub use interval::Interval;
pub use lanes::Lanes;
pub use mask::{LaneMask, Mask};
pub use predicate::{Predicate, Ring, Side, Tally, Verdict, decide_computed, decide_stored};
pub use real::Real;
pub use trig::{atan2, cos, sin, sin_cos};
