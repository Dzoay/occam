//! Source: original
//! What a step gives beyond its operation and label, read and checked when the request is, before any geometry, and
//! converted once. Each operation's step adds its arm as it lands.

use occam_report::Report;
use serde_json::{Map, Value};

use crate::construct::given::{self, FaceArgs, WireArgs};
use crate::operation::Operation;
use crate::primitive::{self, PrimitiveArgs};
use crate::read::At;
use crate::request::Units;

/// What a step gives for its operation, checked and converted.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Args {
    /// Nothing read here: an operation whose step reads nothing more, or none yet.
    #[default]
    None,
    /// A wire's edges.
    Wire(WireArgs),
    /// A face's wires.
    Face(FaceArgs),
    /// A primitive's shape and frame.
    Primitive(PrimitiveArgs),
}

/// What the step at `at`, asking for `operation`, gives in `fields`, its fields other than `op` and `label`, with every
/// bad input in `out`.
pub(crate) fn read(
    at: &At,
    operation: Operation,
    fields: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Args {
    match operation {
        Operation::Wire => given::wire_step(at, fields, units, out).map_or(Args::None, Args::Wire),
        Operation::Face => given::face_step(at, fields, units, out).map_or(Args::None, Args::Face),
        Operation::Box
        | Operation::Cylinder
        | Operation::Sphere
        | Operation::Cone
        | Operation::Torus
        | Operation::Wedge => primitive::read(at, operation, fields, units, out).map_or(Args::None, Args::Primitive),
        Operation::Extrude
        | Operation::Revolve
        | Operation::Transform
        | Operation::Copy
        | Operation::Measure
        | Operation::Boolean
        | Operation::Fillet
        | Operation::Chamfer => Args::None,
    }
}
