//! Source: original
//! The owned trigonometry (S5, O14): the same bits on every target, lane width and build, because it is written over
//! [`Real`](crate::Real) with `+ - * /` and integer operations only. Its coefficients are occam's own fit (O15).

mod atan2;
mod constants;
mod large;
mod sincos;

pub use atan2::atan2;
pub use sincos::{cos, sin, sin_cos};
