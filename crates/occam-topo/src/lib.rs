//! Source: original
//! The B-rep (PARITY 1.1): bodies, shells, faces, loops, wires, edge uses, edges and vertices, and the curves and
//! surfaces they lie on.
//!
//! Every entity has an id of slot and generation, with its kind as a type ([`Id`]). Records sit in copy-on-write chunks,
//! one table per kind, so a [`Snapshot`] is immutable and cloning one shares every chunk (S3-P3). A stale id is a
//! `topo.stale-id` report, never the wrong entity.
//!
//! An [`Edit`] is the only place ids are given, and [`Edit::commit`], which checks what the edit touched, is the only way
//! to make a snapshot, so no unchecked model exists. It hands back [`Changes`]: what was created, changed and deleted,
//! and where each new entity came from.

/// This crate by its name, for the fixtures its tests compile from occam-fixtures' source.
#[cfg(test)]
extern crate self as occam_topo;

mod check;
mod digest;
mod edit;
mod geometric;
mod kind;
/// The tests' models: occam-fixtures' source, compiled into the tests alone, where it names this crate as any caller
/// does.
#[cfg(test)]
#[path = "../../occam-fixtures/src/topo/models/mod.rs"]
#[allow(dead_code, reason = "not every test model is used by this crate's own tests")]
mod models;
#[cfg(test)]
mod probe;
mod records;
mod restore;
mod snapshot;
mod table;
#[cfg(test)]
mod testing;
mod view;
mod whole;

pub use edit::{Changes, Derivation, Edit, Lineage};
pub use geometric::winding_number;
pub use kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Record, Shell, SurfaceGeom, Vertex, Wire,
};
pub use records::{
    Around, BodyKind, BodyRecord, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord,
    ParamCurveRecord, Provenance, Sheet, ShellRecord, StepNote, SurfaceRecord, UseOwner, VertexRecord, WireRecord,
};
pub use restore::{Restore, Saved};
pub use snapshot::Snapshot;
pub use view::View;
