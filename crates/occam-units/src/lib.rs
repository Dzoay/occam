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

mod bounds;
mod direction;
mod frame;
mod measure;
mod placement;
mod tolerance;
mod uv;
mod vec3;

pub use bounds::Bounds;
pub use direction::{Direction, Tangent};
pub use frame::{Displacement, Frame, Local, Point, Position, Role, Triple, Vector, World};
pub use measure::{Angle, Length, Measure, Millimetres, Radians, Unit};
pub use placement::Placement;
pub use tolerance::{ParamKind, Tolerance};
pub use uv::{Pair, UvPoint, UvVector};
pub use vec3::Vec3;
