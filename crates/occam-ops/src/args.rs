//! Source: original
//! What a step gives beyond its operation and label, read and checked when the request is, before any geometry, and
//! converted once, by the reader its operation's row names (`operation.rs`).

use occam_report::Report;
use serde_json::{Map, Value};

use crate::construct::given::{FaceArgs, WireArgs};
use crate::measure::MeasureArgs;
use crate::operation::Operation;
use crate::primitive::PrimitiveArgs;
use crate::read::At;
use crate::request::Units;
use crate::sweep::{ExtrudeArgs, RevolveArgs};
use crate::transform::TransformArgs;

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
    /// An extrusion's face and vector.
    Extrude(ExtrudeArgs),
    /// A revolution's profile, axis and angle.
    Revolve(RevolveArgs),
    /// What a measure measures, and what it wants of it.
    Measure(MeasureArgs),
    /// The body a transform or a copy moves, and the map.
    Transform(TransformArgs),
}

/// What the step at `at`, asking for `operation`, gives in `fields`, its fields other than `op` and `label`, as the
/// operation's own reader reads them, with every bad input in `out`.
pub(crate) fn read(
    at: &At,
    operation: Operation,
    fields: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Args {
    operation.does().and_then(|does| (does.read)(at, operation, fields, units, out)).unwrap_or_default()
}
