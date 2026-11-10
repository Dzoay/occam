//! `occam-io`: the native format and STEP.

use occam_report::Report;

use crate::math::Exec;
use crate::topo::{Body, Id, Snapshot};

/// The native format: the records as JSON, which is also the readable dump (PARITY 2.17, L8). It carries a schema
/// version from the first commit, the tolerance, the step counter and every slot's generation, so ids survive a save and
/// a load and a loaded model hashes as the saved one did.
///
/// # Errors
///
/// A report naming the record that would not encode, such as a geometry family with no encoding yet.
pub fn save(model: &Snapshot) -> Result<String, Report> {
    unimplemented!()
}

/// Loads and checks the whole model (`Snapshot::check_all`): nothing from outside is trusted.
///
/// # Errors
///
/// Every failure, each naming its record.
pub fn load(text: &str, exec: &Exec) -> Result<Snapshot, Vec<Report>> {
    unimplemented!()
}

/// STEP export of solids (PARITY 1.9). Anything that cannot be written is reported, never dropped.
///
/// # Errors
///
/// One report per body that could not be written.
pub fn export_step(model: &Snapshot, bodies: &[Id<Body>]) -> Result<String, Vec<Report>> {
    unimplemented!()
}
