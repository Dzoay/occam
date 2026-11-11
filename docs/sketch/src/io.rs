//! `occam-io`: the native format and STEP.

use occam_report::Report;

use occam_math::Exec;
use crate::topo::{Body, Id, Snapshot};

/// The native format: the records as JSON, which is also the readable dump (PARITY 2.17, L8). It carries a schema
/// version from the first commit, the tolerance, the step counter and every slot's generation, so ids survive a save and
/// a load and a loaded model hashes as the saved one did.
///
/// Bytes, not text: JSON is UTF-8, so nothing changes today, and a binary encoding, added if the `construction` exit
/// finds JSON too slow, needs no new signature.
///
/// # Errors
///
/// A report naming the record that would not encode, such as a geometry family with no encoding yet.
pub fn save(model: &Snapshot) -> Result<Vec<u8>, Report> {
    unimplemented!()
}

/// Parses, then checks the whole model (`Snapshot::check_all`): nothing from outside is trusted. The `construction`
/// exit times the two steps apart, so a slow load is blamed on the right one.
///
/// # Errors
///
/// Every failure, each naming its record.
pub fn load(bytes: &[u8], exec: &Exec) -> Result<Snapshot, Vec<Report>> {
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
