//! Source: original
//! What the tests build with: occam-fixtures' helpers for the topology, compiled into the tests from its source so that
//! they are of this crate's types, the names the tests have always called its quantities by, and a look inside a
//! snapshot, which only this crate can take.

pub(crate) use occam_fixtures::{point as at, tol as tolerance};
use occam_math::Exec;

use crate::kind::{Id, Kind};
use crate::snapshot::Snapshot;

#[path = "../../occam-fixtures/src/topo/testing.rs"]
mod shared;

pub(crate) use shared::{Unfindable, number, turn_inside_out, turn_loop, turn_over, vertex, x_axis};

/// One thread.
pub(crate) const fn serial() -> Exec {
    Exec::serial()
}

/// The live record of `id` in `snapshot`.
pub(crate) fn record<K: Kind>(snapshot: &Snapshot, id: Id<K>) -> Option<&K::Record> {
    K::table(&snapshot.model).live(id.slot(), id.generation())
}
