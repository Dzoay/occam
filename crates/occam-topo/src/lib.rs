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

mod check;
mod digest;
mod edit;
mod geometric;
mod kind;
#[cfg(test)]
mod models;
/// The tests' models, built outside the tests only for the determinism probe and the workloads, which nothing that ships
/// turns on.
#[cfg(all(any(feature = "determinism", feature = "workloads"), not(test)))]
#[allow(
    dead_code,
    unused_imports,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "the tests' models, as they are written"
)]
mod models;
#[cfg(any(test, feature = "determinism"))]
#[doc(hidden)]
pub mod probe;
mod records;
mod snapshot;
mod table;
#[cfg(test)]
mod testing;
/// What the tests share, which the models use, so it comes with them for the determinism probe and the workloads.
#[cfg(all(any(feature = "determinism", feature = "workloads"), not(test)))]
#[allow(dead_code, clippy::unwrap_used, reason = "what the tests share, as they are written")]
mod testing;
mod view;
mod whole;
#[cfg(any(test, feature = "workloads"))]
#[doc(hidden)]
pub mod workloads;

pub use edit::{Changes, Derivation, Edit, Lineage};
pub use kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Record, Shell, SurfaceGeom, Vertex, Wire,
};
pub use records::{
    Around, BodyKind, BodyRecord, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord,
    ParamCurveRecord, Provenance, Sheet, ShellRecord, SurfaceRecord, UseOwner, VertexRecord, WireRecord,
};
pub use snapshot::Snapshot;
pub use view::View;
