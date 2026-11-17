//! Source: original
//! A primitive's parameters, read and checked with the rest of the request, every bad input reported with its place
//! (S6), and converted to millimetres once, into what the step that makes the primitive reads.
//!
//! Every primitive takes an optional `frame`: an `origin`, and the directions of its `z` and `x` axes, by default the
//! world's. Sizes are lengths greater than zero; a cone's radii may be zero, but not both and not equal; a torus's
//! minor radius is less than its major; a wedge's top is a strip of its `width`, zero or more, from `start` along `x`.

use occam_report::{Code, LengthUnit, NamedValue, Report};
use occam_units::{Length, Vec3};
use serde_json::{Map, Value};

use super::pose::Pose;
use crate::operation::Operation;
use crate::read::{At, gather, needed, number, object, wrong_type};
use crate::request::Units;

/// How close to `z` an `x` axis may lie, as the sine of the angle between them: the default angle tolerance (O12).
const PARALLEL: f64 = 1e-9;

/// What a primitive is, its sizes in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A box of these sizes along `x`, `y` and `z`.
    Box([f64; 3]),
    /// A cylinder.
    Cylinder {
        /// Its radius.
        radius: f64,
        /// Its height, along `z`.
        height: f64,
    },
    /// A sphere.
    Sphere {
        /// Its radius.
        radius: f64,
    },
    /// A cone, which may come to a point at either end but not both.
    Cone {
        /// Its radius at its base, where `z` is 0.
        radius1: f64,
        /// Its radius at its top.
        radius2: f64,
        /// Its height, along `z`.
        height: f64,
    },
    /// A torus about `z`.
    Torus {
        /// The radius of the circle its tube's centre runs round.
        major: f64,
        /// The radius of its tube, less than `major`.
        minor: f64,
    },
    /// A wedge: a box whose top is narrowed to a strip along `x`.
    Wedge {
        /// The sizes of its base along `x` and `y`, and its height along `z`.
        size: [f64; 3],
        /// Where its top begins along `x`.
        start: f64,
        /// How wide its top is along `x`, zero or more.
        width: f64,
    },
}

/// A primitive a step makes: what it is and where, read and converted when the request is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimitiveArgs {
    /// What it is.
    pub shape: Shape,
    /// Where it is.
    pub pose: Pose,
}

/// The length `value` at `at`, in `unit`, in millimetres.
fn length(at: &At, value: &Value, unit: LengthUnit) -> Result<f64, Report> {
    let given = number(at, value)?;
    Length::new(given, unit).map(Length::millimetres).map_err(|report| report.with_input(at.input()))
}

/// The length named `name` of `fields`, which is needed.
fn field(at: &At, fields: &Map<String, Value>, name: &str, unit: LengthUnit) -> Result<f64, Report> {
    length(&at.field(name), needed(at, fields, name)?, unit)
}

/// `size` at `at`, which must be greater than zero.
fn positive(at: &At, size: f64) -> Result<f64, Report> {
    if size > 0.0 {
        Ok(size)
    } else {
        Err(at.report(Code::SizeNotPositive).with_value(NamedValue::length("size", size)))
    }
}

/// `size` at `at`, which must not be less than zero.
fn not_negative(at: &At, size: f64) -> Result<f64, Report> {
    if size >= 0.0 {
        Ok(size)
    } else {
        Err(at.report(Code::InputNegative).with_value(NamedValue::length("value", size)))
    }
}

/// The size named `name` of `fields`, needed and greater than zero.
fn size(at: &At, fields: &Map<String, Value>, name: &str, unit: LengthUnit) -> Result<f64, Report> {
    positive(&at.field(name), field(at, fields, name, unit)?)
}

/// The three numbers of the list at `at`.
pub(crate) fn three(at: &At, value: &Value) -> Result<[(At, f64); 3], Report> {
    let Value::Array(items) = value else { return Err(wrong_type(at, value, "list")) };
    let [a, b, c] = items.as_slice() else {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        return Err(at
            .report(Code::WrongCount)
            .with_value(NamedValue::count("found", count(items.len())))
            .with_value(NamedValue::count("needed", 3)));
    };
    let read = |k: usize, v: &Value| number(&at.item(k), v).map(|x| (at.item(k), x));
    Ok([read(0, a)?, read(1, b)?, read(2, c)?])
}

/// The three lengths of the list at `at`, in `unit`, in millimetres.
fn lengths(at: &At, value: &Value, unit: LengthUnit) -> Result<[f64; 3], Report> {
    let [a, b, c] = three(at, value)?;
    let convert =
        |(at, x): (At, f64)| Length::new(x, unit).map(Length::millimetres).map_err(|r| r.with_input(at.input()));
    Ok([convert(a)?, convert(b)?, convert(c)?])
}

/// The three sizes of the list at `at`, each greater than zero.
fn sizes(at: &At, value: &Value, unit: LengthUnit, out: &mut Vec<Report>) -> Option<[f64; 3]> {
    let [a, b, c] = gather(lengths(at, value, unit), out)?;
    let [a, b, c] = [(0, a), (1, b), (2, c)].map(|(k, x)| gather(positive(&at.item(k), x), out));
    Some([a?, b?, c?])
}

/// The unit direction of the list at `at`, which must be finite and not zero.
fn direction(at: &At, value: &Value) -> Result<Vec3, Report> {
    let [(_, x), (_, y), (_, z)] = three(at, value)?;
    let v = Vec3::new(x, y, z);
    let size = v.norm();
    if size.is_finite() && size > 0.0 { Ok(v * (1.0 / size)) } else { Err(at.report(Code::InputNoDirection)) }
}

/// `x`'s part across the unit `z`, as a unit direction, if it has one.
fn across(x: Vec3, z: Vec3) -> Option<Vec3> {
    let part = x - z * x.dot(z);
    let size = part.norm();
    (size > PARALLEL).then(|| part * (1.0 / size))
}

/// The frame at `at`, if the step gives one, or the world's.
pub(crate) fn pose(at: &At, given: Option<&Value>, unit: LengthUnit, out: &mut Vec<Report>) -> Option<Pose> {
    let Some(given) = given else { return Some(Pose::STANDARD) };
    let fields = object(at, given, &["origin", "z", "x"], out)?;
    let read = |name: &str, f: &dyn Fn(&At, &Value) -> Result<Vec3, Report>, default: Vec3| {
        fields.get(name).map_or(Ok(default), |value| f(&at.field(name), value))
    };
    let origin = gather(
        read("origin", &|at, v| lengths(at, v, unit).map(|[a, b, c]| Vec3::new(a, b, c)), Pose::STANDARD.origin),
        out,
    );
    let z = gather(read("z", &direction, Pose::STANDARD.z), out);
    let x = gather(read("x", &direction, Pose::STANDARD.x), out);
    let (origin, z, x) = (origin?, z?, x?);
    let x = match (fields.contains_key("x"), across(x, z)) {
        (_, Some(x)) => x,
        (false, None) => across(Vec3::new(0.0, 1.0, 0.0), z)?,
        (true, None) => {
            out.push(at.field("x").report(Code::InputAxesParallel));
            return None;
        }
    };
    Some(Pose { origin, z, x })
}

/// A cone's two radii, which may be zero but not both, and not equal. Equal means equal as given: radii that differ at
/// all make a cone, whose half-angle is then small but more than zero.
#[expect(clippy::float_cmp, reason = "the radii as given, compared exactly")]
fn radii(at: &At, fields: &Map<String, Value>, unit: LengthUnit, out: &mut Vec<Report>) -> Option<(f64, f64)> {
    let mut radius =
        |name: &str| gather(field(at, fields, name, unit).and_then(|r| not_negative(&at.field(name), r)), out);
    let (one, two) = (radius("radius1"), radius("radius2"));
    let (one, two) = (one?, two?);
    if one == two {
        out.push(at.field("radius2").report(Code::InputConeRadiiEqual).with_value(NamedValue::length("value", two)));
        return None;
    }
    Some((one, two))
}

/// A torus's two radii, the minor less than the major.
fn torus(at: &At, fields: &Map<String, Value>, unit: LengthUnit, out: &mut Vec<Report>) -> Option<Shape> {
    let major = gather(size(at, fields, "major", unit), out);
    let minor = gather(size(at, fields, "minor", unit), out);
    let (major, minor) = (major?, minor?);
    if minor >= major {
        let report = at
            .field("minor")
            .report(Code::OutOfRange)
            .with_value(NamedValue::length("value", minor))
            .with_value(NamedValue::length("low", 0.0))
            .with_value(NamedValue::length("high", major));
        out.push(report);
        return None;
    }
    Some(Shape::Torus { major, minor })
}

/// A wedge's sizes and its top.
fn wedge(at: &At, fields: &Map<String, Value>, unit: LengthUnit, out: &mut Vec<Report>) -> Option<Shape> {
    let size = gather(needed(at, fields, "size"), out).and_then(|value| sizes(&at.field("size"), value, unit, out));
    let top_at = at.field("top");
    let top = gather(needed(at, fields, "top"), out).and_then(|value| object(&top_at, value, &["start", "width"], out));
    let top = top.and_then(|top| {
        let start = gather(field(&top_at, top, "start", unit), out);
        let width =
            gather(field(&top_at, top, "width", unit).and_then(|w| not_negative(&top_at.field("width"), w)), out);
        Some((start?, width?))
    });
    let (size, (start, width)) = (size?, top?);
    Some(Shape::Wedge { size, start, width })
}

/// The shape a step making `operation` asks for, from its `fields`.
fn shape(
    at: &At,
    operation: Operation,
    fields: &Map<String, Value>,
    unit: LengthUnit,
    out: &mut Vec<Report>,
) -> Option<Shape> {
    let size = |name: &str, out: &mut Vec<Report>| gather(size(at, fields, name, unit), out);
    match operation {
        Operation::Box => {
            gather(needed(at, fields, "size"), out).and_then(|v| sizes(&at.field("size"), v, unit, out)).map(Shape::Box)
        }
        Operation::Cylinder => {
            let (radius, height) = (size("radius", out), size("height", out));
            Some(Shape::Cylinder { radius: radius?, height: height? })
        }
        Operation::Sphere => size("radius", out).map(|radius| Shape::Sphere { radius }),
        Operation::Cone => {
            let (radii, height) = (radii(at, fields, unit, out), size("height", out));
            let ((radius1, radius2), height) = (radii?, height?);
            Some(Shape::Cone { radius1, radius2, height })
        }
        Operation::Torus => torus(at, fields, unit, out),
        Operation::Wedge => wedge(at, fields, unit, out),
        _ => None,
    }
}

/// The primitive the step at `at`, making `operation`, asks for with `fields` in `units`, every bad input in `out`.
pub(crate) fn read(
    at: &At,
    operation: Operation,
    fields: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Option<PrimitiveArgs> {
    let shape = shape(at, operation, fields, units.length, out);
    let pose = pose(&at.field("frame"), fields.get("frame"), units.length, out);
    Some(PrimitiveArgs { shape: shape?, pose: pose? })
}
