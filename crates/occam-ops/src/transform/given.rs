//! Source: original
//! What a `transform` and a `copy` step give, read and checked whole when the request is, and converted once into one
//! map. The `target` is a body, an earlier step's label or an id. An optional `scale` comes first: one factor, or three
//! along the X, Y and Z axes of an optional `about` frame, about its origin, each more than zero. An optional `frame`,
//! where the world's frame is placed, comes after it. Both frames are an origin and the directions of their `z` and `x`
//! axes, read as a primitive's frame is, and either defaults to the world's
//! (`docs/decisions/2026-11-12-non-uniform-scale.md`).

use occam_report::{Code, Input, NamedValue, Quantity, Report};
use occam_units::{Affine, Local, Placement, World};
use serde_json::{Map, Value};

use crate::args::Args;
use crate::operation::Operation;
use crate::primitive::{pose, three};
use crate::read::{At, gather, number, wrong_type};
use crate::request::Units;
use crate::sweep::{Named, named};

/// A transform or a copy as a request gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformArgs {
    /// The body it moves, or copies.
    pub target: Named,
    /// The map, in millimetres: the scale about its frame, then the rigid frame.
    pub map: Affine,
    /// Where the scale is in the request, which a refusal of a form with no exact image under it names.
    pub scale_at: Input,
}

/// The factor `x` at `at`, if it is finite and more than zero.
fn positive(at: &At, x: f64) -> Result<f64, Report> {
    if x.is_finite() && x > 0.0 {
        Ok(x)
    } else {
        Err(at.report(Code::InputScaleNotPositive).with_value(NamedValue::new("value", Quantity::Ratio, x)))
    }
}

/// The scale's factors at `at`: one number for all three, or a list of three, each finite and more than zero, every bad
/// one in `out`.
fn factors(at: &At, value: &Value, out: &mut Vec<Report>) -> Option<[f64; 3]> {
    if value.is_number() {
        let factor = gather(number(at, value).and_then(|x| positive(at, x)), out)?;
        return Some([factor; 3]);
    }
    if !value.is_array() {
        out.push(wrong_type(at, value, "number or list"));
        return None;
    }
    let [a, b, c] = gather(three(at, value), out)?.map(|(at, x)| gather(positive(&at, x), out));
    Some([a?, b?, c?])
}

/// What a `transform` or a `copy` step, at `at`, gives in `fields`.
pub(crate) fn transform_step(
    at: &At,
    _: Operation,
    fields: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Option<Args> {
    let target = gather(named(at, fields, "target"), out);
    let scale_at = at.field("scale");
    let factors = fields.get("scale").map_or(Some([1.0; 3]), |value| factors(&scale_at, value, out));
    let about = pose(&at.field("about"), fields.get("about"), units.length, out);
    let frame = pose(&at.field("frame"), fields.get("frame"), units.length, out);
    let (target, factors, about, frame) = (target?, factors?, about?, frame?);
    let placed = Placement::<World, Local>::identity().then(&frame.placement());
    let map = Affine::stretch(&about.placement(), factors)?.then(&Affine::placed(placed));
    Some(Args::Transform(TransformArgs { target, map, scale_at: scale_at.input() }))
}
