//! Source: original
//! The one maths implementation (L23): every number the kernel computes goes through this crate.
//!
//! [`Real`] is the number type maths is written over, once, and is run as `f64` or as [`Lanes`] of `f64`. Same inputs give
//! identical bits on every target, at every thread count, scalar and in lanes. `docs/DETERMINISM.md` is the contract, and
//! `gates determinism` holds it.

mod hash;
mod lanes;
mod real;
mod splitmix;
pub mod stream;

pub use hash::{CANONICAL_NAN, Digest, canonical_bits};
pub use lanes::Lanes;
pub use real::Real;
