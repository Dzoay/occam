//! `occam`, the composition crate (L25): what it gains after `construction.request`, which made the request runner real
//! (`occam::run`), and `construction.describer`, which made the one resolver of descriptions real (`occam::describe`, and
//! the descriptions every run's result carries): the listing, and the session the facades wrap. `occam-cli`, `occam-wasm` and the later `occam-capi` and `occam-mcp` bind this and nothing else.

use std::sync::Arc;

use occam_math::Exec;
use occam_report::EntityRef;

use occam_topo::Snapshot;

/// What an agent asks between steps (PARITY 1.1): every entity of a kind. What any of them is, and its neighbours, is
/// `occam::describe`. Each takes many ids and gives one result per id (L6).
#[must_use]
pub fn list(model: &Snapshot, kind: occam_report::EntityKind) -> Vec<EntityRef> {
    unimplemented!()
}

/// What `occam-wasm` wraps, one per worker (P10). The editing worker runs requests and publishes the native bytes after
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
    pub fn open(native: &[u8]) -> Result<Self, Vec<occam_report::Report>> {
        unimplemented!()
    }

    /// The request's results as JSON, with the new model's native bytes when it changed.
    #[must_use]
    pub fn run(&mut self, request: &str) -> (String, Option<Vec<u8>>) {
        unimplemented!()
    }
}
