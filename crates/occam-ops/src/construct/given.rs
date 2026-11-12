//! Source: original
//! What a `wire` and a `face` step give, read and checked whole when the request is, and converted once to millimetres.
//!
//! A wire is straight edges through points, back to the first if it is `closed`, or edges one by one: a `line` between two
//! points, an `arc` from a point through a second to a third, or a whole `circle` about a centre. A face names its outer
//! wire and its holes, each an earlier `wire` step's label, a wire's id, or a wire written in place.

use occam_report::{Code, Fault, Input, NamedValue, Quantity, Report};
use occam_units::{Length, Vec3};
use serde_json::{Map, Value};

use crate::operand::Operand;
use crate::read::{At, missing, needed, number, object, wrong_type};
use crate::request::Units;

/// One edge of a wire as a request gives it, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    /// The straight edge from `from` to `to`.
    Line {
        /// Where it begins.
        from: Vec3,
        /// Where it ends.
        to: Vec3,
    },
    /// The arc from `from` through `through` to `to`.
    Arc {
        /// Where it begins.
        from: Vec3,
        /// A point it passes through between its ends.
        through: Vec3,
        /// Where it ends.
        to: Vec3,
    },
    /// The whole circle about `centre` of `radius`, anticlockwise about `normal`.
    Circle {
        /// Its centre.
        centre: Vec3,
        /// Its radius, above zero.
        radius: f64,
        /// The way it faces, not of length zero.
        normal: Vec3,
    },
}

/// A wire as a request gives it.
#[derive(Clone, Debug, PartialEq)]
pub enum WireArgs {
    /// Straight edges through points, each with where it is in the request; back to the first if `closed`.
    Points {
        /// The points.
        points: Vec<(Vec3, Input)>,
        /// Whether the last joins the first.
        closed: bool,
        /// Where the wire is in the request.
        at: Input,
    },
    /// Edges one by one, each with where it is in the request.
    Edges {
        /// The edges.
        edges: Vec<(Piece, Input)>,
        /// Where the wire is in the request.
        at: Input,
    },
}

impl WireArgs {
    /// Where the wire is in the request.
    #[must_use]
    pub const fn at(&self) -> &Input {
        match self {
            Self::Points { at, .. } | Self::Edges { at, .. } => at,
        }
    }
}

/// A wire a face is bounded by.
#[derive(Clone, Debug, PartialEq)]
pub enum WireRef {
    /// An earlier step's wire, or a wire of the model, and where it is named.
    Named(Operand, Input),
    /// A wire written in place.
    Given(WireArgs),
}

/// A face as a request gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceArgs {
    /// The wire round its outside.
    pub outer: WireRef,
    /// The wires round its holes.
    pub holes: Vec<WireRef>,
}

/// The value of `result`, or nothing with its report kept.
fn gather<T>(result: Result<T, Report>, out: &mut Vec<Report>) -> Option<T> {
    result.map_err(|report| out.push(report)).ok()
}

/// The point at `at`, three numbers in the request's length unit, in millimetres.
fn point(at: &At, value: &Value, units: Units) -> Result<Vec3, Report> {
    let Some([x, y, z]) = value.as_array().and_then(|items| <&[Value; 3]>::try_from(items.as_slice()).ok()) else {
        return Err(wrong_type(at, value, "point of three numbers"));
    };
    let coordinate = |k: usize, value: &Value| {
        let at = at.item(k);
        let given = number(&at, value)?;
        Length::new(given, units.length).map(Length::millimetres).map_err(|report| report.with_input(at.input()))
    };
    Ok(Vec3::new(coordinate(0, x)?, coordinate(1, y)?, coordinate(2, z)?))
}

/// The direction at `at`, three numbers of no unit, not all zero.
fn direction(at: &At, value: &Value) -> Result<Vec3, Report> {
    let Some([x, y, z]) = value.as_array().and_then(|items| <&[Value; 3]>::try_from(items.as_slice()).ok()) else {
        return Err(wrong_type(at, value, "direction of three numbers"));
    };
    let v = Vec3::new(number(&at.item(0), x)?, number(&at.item(1), y)?, number(&at.item(2), z)?);
    let length = v.norm();
    if length.is_finite() && length > 0.0 {
        return Ok(v);
    }
    Err(Report::new(Code::DegenerateDirection, Fault::InvalidInput)
        .with_input(at.input())
        .with_value(NamedValue::new("length", Quantity::Ratio, length))
        .with_value(NamedValue::length("tolerance", 0.0)))
}

/// The list at `at` of `count` points.
fn points<const N: usize>(at: &At, value: &Value, units: Units) -> Result<[Vec3; N], Report> {
    let wanted = || format!("list of {N} points");
    let items = value.as_array().filter(|items| items.len() == N).ok_or_else(|| wrong_type(at, value, &wanted()))?;
    let read: Result<Vec<Vec3>, Report> = items.iter().enumerate().map(|(k, v)| point(&at.item(k), v, units)).collect();
    <[Vec3; N]>::try_from(read?).map_err(|_| wrong_type(at, value, &wanted()))
}

/// The whole circle at `at`: its centre, radius and normal.
fn circle(at: &At, value: &Value, units: Units, out: &mut Vec<Report>) -> Option<Piece> {
    let fields = object(at, value, &["centre", "radius", "normal"], out)?;
    let centre = gather(needed(at, fields, "centre").and_then(|v| point(&at.field("centre"), v, units)), out);
    let radius = gather(needed(at, fields, "radius").and_then(|v| radius(&at.field("radius"), v, units)), out);
    let normal = gather(needed(at, fields, "normal").and_then(|v| direction(&at.field("normal"), v)), out);
    Some(Piece::Circle { centre: centre?, radius: radius?, normal: normal? })
}

/// The radius at `at`: a length above zero, in millimetres.
fn radius(at: &At, value: &Value, units: Units) -> Result<f64, Report> {
    let given = number(at, value)?;
    let length = Length::new(given, units.length).map_err(|report| report.with_input(at.input()))?.millimetres();
    if length > 0.0 {
        return Ok(length);
    }
    Err(Report::new(Code::SizeNotPositive, Fault::InvalidInput)
        .with_input(at.input())
        .with_value(NamedValue::length("size", length)))
}

/// The edge at `at`: an object of one field, `line`, `arc` or `circle`.
fn piece(at: &At, value: &Value, units: Units, out: &mut Vec<Report>) -> Option<Piece> {
    let fields = object(at, value, &["line", "arc", "circle"], out)?;
    let kinds: Vec<&String> = fields.keys().collect();
    let [kind] = kinds.as_slice() else {
        out.push(one_of(at, &kinds, "line, arc, circle"));
        return None;
    };
    let (at, given) = (at.field(kind), fields.get(kind.as_str())?);
    match kind.as_str() {
        "line" => gather(points::<2>(&at, given, units), out).map(|[from, to]| Piece::Line { from, to }),
        "arc" => {
            gather(points::<3>(&at, given, units), out).map(|[from, through, to]| Piece::Arc { from, through, to })
        }
        _ => circle(&at, given, units, out),
    }
}

/// `given` names, at `at`, where exactly one of `allowed` is wanted.
fn one_of(at: &At, given: &[&String], allowed: &str) -> Report {
    let names: Vec<&str> = given.iter().map(|name| name.as_str()).collect();
    let given = if names.is_empty() { "none of them".to_string() } else { names.join(", ") };
    Report::new(Code::InputOneOf, Fault::InvalidInput).with_input(at.input().given(&given).expected(allowed))
}

/// The list at `at`, each item read by `item`.
fn list<T>(at: &At, value: &Value, mut item: impl FnMut(&At, &Value) -> Option<T>) -> Result<Vec<T>, Report> {
    let items = value.as_array().ok_or_else(|| wrong_type(at, value, "list"))?;
    Ok(items.iter().enumerate().filter_map(|(k, v)| item(&at.item(k), v)).collect())
}

/// The wire whose fields are `fields`, at `at`: `points` and, optionally, `closed`, or `edges`.
pub(crate) fn wire(at: &At, fields: &Map<String, Value>, units: Units, out: &mut Vec<Report>) -> Option<WireArgs> {
    let kinds: Vec<&String> = fields.keys().filter(|name| matches!(name.as_str(), "points" | "edges")).collect();
    match kinds.as_slice() {
        [kind] if kind.as_str() == "points" => through_points(at, fields, units, out),
        [_] => {
            let given = fields.get("edges")?;
            let edges = list(&at.field("edges"), given, |at, v| piece(at, v, units, out).map(|p| (p, at.input())));
            let edges = gather(edges, out)?;
            if given.as_array().is_some_and(Vec::is_empty) {
                out.push(count_wrong(&at.field("edges"), 0, 1));
            }
            Some(WireArgs::Edges { edges, at: at.input() })
        }
        _ => {
            out.push(one_of(at, &kinds, "points, edges"));
            None
        }
    }
}

/// Straight edges through the points at `at`'s `points`, closed if its `closed` says so.
fn through_points(at: &At, fields: &Map<String, Value>, units: Units, out: &mut Vec<Report>) -> Option<WireArgs> {
    let closed = match fields.get("closed") {
        Some(Value::Bool(closed)) => *closed,
        Some(other) => {
            out.push(wrong_type(&at.field("closed"), other, "true or false"));
            false
        }
        None => false,
    };
    let given = fields.get("points")?;
    let read = list(&at.field("points"), given, |at, v| gather(point(at, v, units), out).map(|p| (p, at.input())));
    let points = gather(read, out)?;
    let (given_points, needed_points) = (given.as_array().map_or(0, Vec::len), if closed { 3 } else { 2 });
    if given_points < needed_points {
        out.push(count_wrong(&at.field("points"), given_points, needed_points));
    }
    Some(WireArgs::Points { points, closed, at: at.input() })
}

/// A list at `at` of `found` items where `needed` are needed.
fn count_wrong(at: &At, found: usize, needed: usize) -> Report {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Report::new(Code::WrongCount, Fault::InvalidInput)
        .with_input(at.input())
        .with_value(NamedValue::count("found", count(found)))
        .with_value(NamedValue::count("needed", count(needed)))
}

/// The wire a face names at `at`: an operand, or a wire written in place.
fn wire_ref(at: &At, value: &Value, units: Units, out: &mut Vec<Report>) -> Option<WireRef> {
    match value {
        Value::String(name) => Some(WireRef::Named(Operand::read(name), at.input())),
        Value::Object(_) => {
            let fields = object(at, value, &["points", "closed", "edges"], out)?;
            wire(at, fields, units, out).map(WireRef::Given)
        }
        _ => {
            out.push(wrong_type(at, value, "label, id or wire"));
            None
        }
    }
}

/// The face whose fields are `fields`, at `at`: its `outer` wire and, optionally, its `holes`.
pub(crate) fn face(at: &At, fields: &Map<String, Value>, units: Units, out: &mut Vec<Report>) -> Option<FaceArgs> {
    let outer = if let Some(value) = fields.get("outer") {
        wire_ref(&at.field("outer"), value, units, out)
    } else {
        out.push(missing(&at.field("outer")));
        None
    };
    let holes = match fields.get("holes") {
        Some(value) => gather(list(&at.field("holes"), value, |at, v| wire_ref(at, v, units, out)), out)?,
        None => Vec::new(),
    };
    Some(FaceArgs { outer: outer?, holes })
}

/// The fields of a `wire` step, at `at`, checking that it takes them.
pub(crate) fn wire_step(at: &At, fields: &Map<String, Value>, units: Units, out: &mut Vec<Report>) -> Option<WireArgs> {
    let value = Value::Object(fields.clone());
    let fields = object(at, &value, &["points", "closed", "edges"], out)?;
    wire(at, fields, units, out)
}

/// The fields of a `face` step, at `at`, checking that it takes them.
pub(crate) fn face_step(at: &At, fields: &Map<String, Value>, units: Units, out: &mut Vec<Report>) -> Option<FaceArgs> {
    let value = Value::Object(fields.clone());
    let fields = object(at, &value, &["outer", "holes"], out)?;
    face(at, fields, units, out)
}
