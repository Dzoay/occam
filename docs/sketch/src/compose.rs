//! `occam`, the composition crate (L25): the request runner, the one resolver of descriptions, and the session the
//! facades wrap. `occam-cli`, `occam-wasm` and the later `occam-capi` and `occam-mcp` bind this and nothing else.

use std::sync::Arc;

use crate::math::Exec;
use crate::ops::{Ran, Request};
use crate::report::{Details, EntityRef};
use crate::topo::{Describe, Snapshot};

/// Runs a request against a model, or against a new one.
#[must_use]
pub fn run(exec: &Exec, model: Option<Arc<Snapshot>>, request: &Request) -> Ran {
    unimplemented!()
}

/// The one place results leave the kernel. Every id in a result's reports, remedies and partial state is described in
/// one batch, against the snapshot and, for a failure, its frozen outputs, and the descriptions go into the report's
/// `described` (O26). A failure site names ids and nothing more, so no site can forget to describe one, and phase
/// outputs stay free of the model's numbering (P8).
pub fn describe_all(reports: &mut [Details], sources: &[&dyn Describe]) {
    unimplemented!()
}

/// What an agent asks between steps (PARITY 1.1): every entity of a kind, the neighbours of one, and what any of them
/// is. Each takes many ids and gives one result per id (L6).
#[must_use]
pub fn list(model: &Snapshot, kind: crate::report::EntityKind) -> Vec<EntityRef> {
    unimplemented!()
}

#[must_use]
pub fn describe(model: &Snapshot, entities: &[EntityRef]) -> Vec<Option<crate::report::Description>> {
    entities.iter().map(|&entity| model.describe(entity)).collect()
}

/// What `occam-wasm` wraps, one per worker (P10). The editing worker runs requests and publishes the native text after
/// each edit; mesh workers load it read-only and mesh ranges of faces. Each call's output is under the determinism
/// contract; the page's assembly of mesh parts is display only, as GPU output is.
pub struct Session {
    model: Arc<Snapshot>,
    exec: Exec,
}

impl Session {
    /// # Errors
    ///
    /// As `occam_io::load`.
    pub fn open(native: &str) -> Result<Self, Vec<occam_report::Report>> {
        unimplemented!()
    }

    /// The request's results as JSON, with the new model's native text when it changed.
    #[must_use]
    pub fn run(&mut self, request: &str) -> (String, Option<String>) {
        unimplemented!()
    }
}
