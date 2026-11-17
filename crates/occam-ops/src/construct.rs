//! Source: original
//! Construction from points and curves (PARITY 1.6): wires through points, lines, arcs and circles, and planar faces
//! bounded by closed wires, with holes. What a request gives is read in [`given`]; the wire and the face are made in
//! [`wire`] and [`face`], checked first in the plane's parameters ([`flat`]) and in space ([`space`], [`outline`]), and
//! written into the edit by [`build`].

mod boxes;
mod build;
mod face;
mod flat;
pub(crate) mod given;
mod loops;
mod outline;
pub(crate) mod space;
mod touch;
mod wire;

use occam_topo::Edit;

pub(crate) use build::{refused, shown};
pub(crate) use wire::band;

use crate::args::Args;
use crate::operand::Labels;
use crate::operation::{Does, Made};

/// A `wire` step: its reader and its runner.
pub(crate) const WIRE: Does = Does { read: given::wire_step, run: run_wire };

/// A `face` step: its reader and its runner.
pub(crate) const FACE: Does = Does { read: given::face_step, run: run_face };

/// Makes the wire `args` holds in `edit`, if it holds one.
fn run_wire(edit: &mut Edit, args: &Args, _: &Labels) -> Option<Made> {
    let Args::Wire(args) = args else { return None };
    Some(wire::wire(edit, args))
}

/// Makes the face `args` holds in `edit`, if it holds one.
fn run_face(edit: &mut Edit, args: &Args, labels: &Labels) -> Option<Made> {
    let Args::Face(args) = args else { return None };
    Some(face::face(edit, args, labels))
}

#[cfg(test)]
pub(crate) mod tests;
