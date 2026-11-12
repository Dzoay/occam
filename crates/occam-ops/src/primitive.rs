//! Source: original
//! The primitives (PARITY 1.12, 2.13): a box, a cylinder, a sphere, a cone, a torus and a wedge, each a closed solid at
//! a frame. Their parameters are read and checked with the rest of the request, before any geometry, and converted once
//! into the step's [`Args::Primitive`](crate::Args::Primitive); the step lays the solid out from them, and `commit`
//! checks it as it checks any edit.

pub(crate) mod parts;
mod polyhedron;
mod pose;
mod read;
mod round;
pub(crate) mod solid;

use occam_report::{EntityRef, Report};
use occam_topo::Edit;
use serde_json::{Map, Value};

use crate::operation::Operation;
use crate::read::At;
use crate::request::Units;
use parts::Parts;

pub use pose::Pose;
pub use read::{PrimitiveArgs, Shape};

/// The primitive the step at `at`, making `operation`, one of the six, asks for with `parameters` in `units`, if every
/// input is good; every bad input in `out`.
pub(crate) fn read(
    at: &At,
    operation: Operation,
    parameters: &Map<String, Value>,
    units: Units,
    out: &mut Vec<Report>,
) -> Option<PrimitiveArgs> {
    let before = out.len();
    read::read(at, operation, parameters, units, out).filter(|_| out.len() == before)
}

/// Makes the primitive `args` describe in `edit`, and gives its body.
///
/// # Errors
///
/// What the edit reports.
pub(crate) fn make(edit: &mut Edit, args: &PrimitiveArgs) -> Result<Option<EntityRef>, Report> {
    let parts = Parts { pose: args.pose.placement() };
    let solid = match args.shape {
        Shape::Box(size) => polyhedron::cuboid(parts, size)?,
        Shape::Wedge { size, start, width } => polyhedron::wedge(parts, size, (start, width))?,
        Shape::Cylinder { radius, height } => round::cylinder(parts, radius, height)?,
        Shape::Cone { radius1, radius2, height } => round::cone(parts, (radius1, radius2), height)?,
        Shape::Sphere { radius } => round::sphere(parts, radius)?,
        Shape::Torus { major, minor } => round::torus(parts, major, minor)?,
    };
    Ok(Some(solid::write(edit, &solid)?.entity()))
}

#[cfg(test)]
mod tests;
