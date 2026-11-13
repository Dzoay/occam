//! Source: original
//! What an `extrude` and a `revolve` step give, read and checked whole when the request is, and converted once. Each
//! names the profile it sweeps, an earlier step's label or an id. An extrusion gives its `vector`, a length in the
//! request's unit; a revolution its axis, through the point `origin` along the direction `axis`, and the `angle` it
//! turns through about it, in the request's angle unit, more than none and at most a whole turn.

use core::f64::consts::TAU;

use occam_report::{Code, Input, NamedValue, Quantity, Report};
use occam_units::{Angle, Vec3};
use serde_json::{Map, Value};

use crate::construct::given::{direction, gather, point};
use crate::construct::space::unit;
use crate::operand::Operand;
use crate::operation::Operation;
use crate::read::{At, needed, number, object, text};
use crate::request::Units;

/// How far past a whole turn an angle may be given and still be read as one, so that 360 degrees converted is a turn.
const PAST_A_TURN: f64 = 1e-12;

/// The profile a sweep names, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    /// The profile.
    pub operand: Operand,
    /// Where the request names it.
    pub at: Input,
}

/// An extrusion as a request gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtrudeArgs {
    /// The face it sweeps.
    pub face: Named,
    /// The vector it sweeps the face along, in millimetres.
    pub vector: Vec3,
    /// Where the vector is in the request.
    pub vector_at: Input,
}

/// A revolution as a request gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct RevolveArgs {
    /// The face or wire it sweeps.
    pub profile: Named,
    /// A point of the axis, in millimetres.
    pub origin: Vec3,
    /// The axis's unit direction, about which the profile turns anticlockwise.
    pub axis: Vec3,
    /// Where the axis is in the request.
    pub axis_at: Input,
    /// The angle it turns through, in radians, more than 0 and at most a whole turn, give or take rounding.
    pub angle: f64,
    /// Where the angle is in the request.
    pub angle_at: Input,
}

/// The profile named by the field `name` of `fields`, at `at`.
pub(crate) fn named(at: &At, fields: &Map<String, Value>, name: &str) -> Result<Named, Report> {
    let at = at.field(name);
    let given = fields.get(name).ok_or_else(|| crate::read::missing(&at))?;
    Ok(Named { operand: Operand::read(text(&at, given)?), at: at.input() })
}

/// The angle at `at`, in `units`, in radians: more than 0 and at most a whole turn.
fn angle(at: &At, value: &Value, units: Units) -> Result<f64, Report> {
    let given = number(at, value)?;
    let radians = Angle::new(given, units.angle).map_err(|report| report.with_input(at.input()))?.radians();
    if radians > 0.0 && radians <= TAU + PAST_A_TURN {
        return Ok(radians);
    }
    Err(at
        .report(Code::OutOfRange)
        .with_value(NamedValue::new("value", Quantity::Angle, radians))
        .with_value(NamedValue::new("low", Quantity::Angle, 0.0))
        .with_value(NamedValue::new("high", Quantity::Angle, TAU)))
}

/// The fields of an `extrude` step, at `at`, checking that it takes them.
pub(crate) fn extrude_step(
    at: &At,
    fields: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Option<ExtrudeArgs> {
    let value = Value::Object(fields.clone());
    let fields = object(at, &value, Operation::Extrude.fields(), out)?;
    let face = gather(named(at, fields, "face"), out);
    let vector_at = at.field("vector");
    let vector = gather(needed(at, fields, "vector").and_then(|v| point(&vector_at, v, units)), out);
    Some(ExtrudeArgs { face: face?, vector: vector?, vector_at: vector_at.input() })
}

/// The fields of a `revolve` step, at `at`, checking that it takes them.
pub(crate) fn revolve_step(
    at: &At,
    fields: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Option<RevolveArgs> {
    let value = Value::Object(fields.clone());
    let fields = object(at, &value, Operation::Revolve.fields(), out)?;
    let profile = gather(named(at, fields, "profile"), out);
    let origin = gather(needed(at, fields, "origin").and_then(|v| point(&at.field("origin"), v, units)), out);
    let axis_at = at.field("axis");
    let axis = gather(needed(at, fields, "axis").and_then(|v| direction(&axis_at, v)), out);
    let angle_at = at.field("angle");
    let angle = gather(needed(at, fields, "angle").and_then(|v| angle(&angle_at, v, units)), out);
    Some(RevolveArgs {
        profile: profile?,
        origin: origin?,
        axis: unit(axis?),
        axis_at: axis_at.input(),
        angle: angle?,
        angle_at: angle_at.input(),
    })
}
