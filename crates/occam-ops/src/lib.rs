//! Source: original
//! Operations as data (docs/architecture/operations.md): a request is read and checked whole, and converted once, before
//! any geometry, with a code for each kind of bad input naming where it is (S6). Its steps then run in order through the
//! edit, in the `occam` crate's runner. What this build does, and what it refuses by name, is [`listing`].

mod args;
mod construct;
mod execute;
#[cfg(test)]
mod facing;
mod guide;
mod measure;
#[cfg(test)]
mod meshed;
mod operand;
mod operation;
mod primitive;
mod read;
mod request;
mod sweep;
mod transform;

pub use args::Args;
pub use construct::given::{FaceArgs, Piece, WireArgs, WireRef};
pub use execute::execute;
pub use guide::guide;
pub use measure::{MeasureArgs, Measured, Measurement, RELATIVE, face_bounds, measure};
pub use operand::{Labels, Operand};
pub use operation::{Operation, listing};
pub use primitive::{Pose, PrimitiveArgs, Shape};
pub use request::{Request, SCHEMA, Step, Units, Unread, default_tolerance};
pub use sweep::{ExtrudeArgs, Named, RevolveArgs};
pub use transform::TransformArgs;
