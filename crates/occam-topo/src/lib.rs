//! Source: original
//! The B-rep (PARITY 1.1): bodies, shells, faces, loops, wires, edge uses, edges and vertices, and the curves and
//! surfaces they lie on.
//!
//! Every entity has an id of slot and generation, with its kind as a type ([`Id`]). Records sit in copy-on-write chunks,
//! one table per kind, so a [`Snapshot`] is immutable and cloning one shares every chunk (S3-P3). A stale id is a
//! `topo.stale-id` report, never the wrong entity.

mod kind;
mod records;
mod snapshot;
#[cfg_attr(not(test), expect(dead_code, reason = "the edit, the next step, writes through these"))]
mod table;

pub use kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Shell, SurfaceGeom, Vertex, Wire,
};
pub use records::{
    Around, BodyKind, BodyRecord, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord,
    ParamCurveRecord, Sheet, ShellRecord, SurfaceRecord, UseOwner, VertexRecord, WireRecord,
};
pub use snapshot::Snapshot;
