//! Source: original
//! Operations as data (docs/architecture/operations.md): a request is read and checked whole, and converted once, before
//! any geometry, with a code for each kind of bad input naming where it is (S6). Its steps then run in order through the
//! edit, in the `occam` crate's runner. What this build does, and what it refuses by name, is [`listing`].

mod args;
mod construct;
mod execute;
mod operand;
mod operation;
mod read;
mod request;

pub use args::Args;
pub use construct::given::{FaceArgs, Piece, WireArgs, WireRef};
pub use execute::execute;
pub use operand::{Labels, Operand};
pub use operation::{Operation, listing};
pub use request::{Request, SCHEMA, Step, Units, default_tolerance};
