//! Source: original
//! Sweeps (PARITY 1.13, 2.15): a planar face extruded along a vector into a solid, and a face or a wire revolved about
//! an axis in its plane into a solid or a sheet. A sweep takes its profile, as a face takes its wires: the profile and
//! its body go, and everything the sweep makes is new, made by the step as the profile's, each point, edge and face
//! with the lineage `swept` to what of the profile it came from. What it makes lies on analytic surfaces wherever one
//! fits, so the native format can save it.

mod around;
mod extrude;
mod given;
mod profile;
mod revolve;
mod shape;

use occam_report::{EntityRef, Report};
use occam_topo::Edit;

pub use given::{ExtrudeArgs, Named, RevolveArgs};
pub(crate) use given::{extrude_step, named, revolve_step};
use profile::Takes;

use crate::construct::band;
use crate::operand::Labels;
use crate::primitive::solid;

/// Makes the extrusion `args` asks for in `edit`, and gives the solid.
///
/// # Errors
///
/// What taking the face reports, and [`occam_report::Code::VectorInPlane`] for a vector that does not leave its plane.
pub(crate) fn extrude(edit: &mut Edit, args: &ExtrudeArgs, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let (band, angle) = (band(edit), edit.base().tolerance().angle().radians());
    let profile = profile::take(edit, &args.face, labels, Takes::Face)?;
    let shape = extrude::extrude(profile, args.vector, (band, angle), &args.vector_at)?;
    Ok(Some(solid::write_from(edit, &shape.solid, &shape.origin)?.entity()))
}

/// Makes the revolution `args` asks for in `edit`, and gives the solid or the sheet.
///
/// # Errors
///
/// What taking the profile reports, and what the revolution refuses: a profile that does not lie in one plane with the
/// axis or lies on both sides of it, and a turn that moves it no more than the band.
pub(crate) fn revolve(edit: &mut Edit, args: &RevolveArgs, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let band = band(edit);
    let profile = profile::take(edit, &args.profile, labels, Takes::FaceOrWire)?;
    let shape = revolve::revolve(profile, args, band)?;
    Ok(Some(solid::write_from(edit, &shape.solid, &shape.origin)?.entity()))
}

#[cfg(test)]
mod tests;
