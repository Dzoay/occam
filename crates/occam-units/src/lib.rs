//! Source: original
//! Typed quantities and frames (L22): a number in the kernel says what it measures and where.
//!
//! Inside the kernel a length is in millimetres and an angle in radians. A number from outside becomes one through
//! [`Length::new`] or [`Angle::new`], which are the one place units are converted and the one place the value is checked.
//! Points and vectors carry their frame in their type, so mixing frames does not compile, and a [`Tolerance`] belongs to a
//! model: there is no default and no global one. All of it is generic over [`occam_math::Real`], so it also runs in lanes.

#![expect(
    clippy::inline_always,
    reason = "a kernel compiles at a dispatched instruction level only through what is inlined into it (P4)"
)]

/// This crate by its name, for the helpers its tests compile from occam-fixtures' source.
#[cfg(test)]
extern crate self as occam_units;

mod affine;
mod bounds;
mod direction;
mod frame;
mod measure;
mod placement;
/// Quantities as the tests write them: occam-fixtures' source, compiled into the tests alone, where it names this crate
/// as any caller does.
#[cfg(test)]
#[path = "../../occam-fixtures/src/units.rs"]
#[allow(dead_code, reason = "not every helper is used by this crate's own tests")]
mod testing;
mod tolerance;
mod uv;
mod vec3;

pub use affine::{Affine, Similar};
pub use bounds::Bounds;
pub use direction::{Direction, Tangent};
pub use frame::{Displacement, Frame, Local, Point, Position, Role, Triple, Vector, World};
pub use measure::{Angle, Length, Measure, Millimetres, Radians, Unit};
pub use placement::Placement;
pub use tolerance::{ParamKind, Tolerance};
pub use uv::{Pair, UvPoint, UvVector};
pub use vec3::Vec3;
