//! Source: original
//! The owned trigonometry (S5, O14): the same bits on every target, lane width and build, because it is written over
//! [`Real`](crate::Real) with `+ - * /` and integer operations only. Its coefficients are occam's own fit (O15).

#[expect(dead_code, reason = "atan2's constants are used from the next step on")]
mod constants;
mod large;
mod sincos;

pub use sincos::{cos, sin, sin_cos};
