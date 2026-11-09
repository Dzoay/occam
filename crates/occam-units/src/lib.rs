//! Source: original
//! Typed quantities and frames (L22): a number in the kernel says what it measures and where.
//!
//! Inside the kernel a length is in millimetres and an angle in radians. A number from outside becomes one through
//! [`Length::new`] or [`Angle::new`], which are the one place units are converted and the one place the value is checked.
//! Points and vectors carry their frame in their type, so mixing frames does not compile, and a [`Tolerance`] belongs to a
//! model: there is no default and no global one. All of it is generic over [`occam_math::Real`], so it also runs in lanes.

mod frame;
mod measure;
mod tolerance;
mod uv;

pub use frame::{Displacement, Frame, Local, Point, Position, Role, Triple, Vector, World};
pub use measure::{Angle, Length, Measure, Millimetres, Radians, Unit};
pub use tolerance::Tolerance;
pub use uv::{Pair, UvPoint, UvVector};
