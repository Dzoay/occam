//! `occam-io`: STEP, which `exchange` adds. The native format's `save` and `load` are real, in the crate.

use occam_report::Report;

use occam_topo::{Body, Id, Snapshot};

/// STEP export of solids (PARITY 1.9). Anything that cannot be written is reported, never dropped.
///
/// # Errors
///
/// One report per body that could not be written.
pub fn export_step(model: &Snapshot, bodies: &[Id<Body>]) -> Result<String, Vec<Report>> {
    unimplemented!()
}
