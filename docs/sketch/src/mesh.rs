//! `occam-mesh`: P9's method. Sample each shared edge once, mesh each face against those samples, one face a task,
//! merge by index; 4.7 to 5.2 times at six threads with the same bits at every thread count and lane width.

use std::ops::Range;
use std::sync::Arc;

use occam_report::Report;
use occam_units::{Angle, Length, Point, World};

use occam_math::{Exec, Grain};
use crate::topo::{Edge, Face, Id, Snapshot};
use occam_units::Direction;

/// Limits come first: without them one collapsed NURBS row took 11.9 GB in seconds (P9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub deflection: Length,
    pub angle: Angle,
    /// No surface may ask a shared edge for a step below this.
    pub floor: Length,
    pub samples_per_edge: u32,
    pub triangles_per_face: u32,
}

/// One edge's samples: read by both faces that use it, which is what removes cracks (P9, P12).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSamples {
    pub edge: Id<Edge>,
    pub params: Vec<f64>,
    pub points: Vec<Point<World>>,
}

/// One face's triangles, with a normal per vertex from the surface, poles included. Along a shared edge the positions
/// are that edge's samples, so neighbouring faces hold the same bits there and a vertex buffer per face needs no
/// welding (P12's buffer layout).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceMesh {
    pub face: Id<Face>,
    pub positions: Vec<Point<World>>,
    pub normals: Vec<Direction<World>>,
    pub triangles: Vec<[u32; 3]>,
    /// Where each boundary edge's samples sit in `positions`, which is what the watertight check reads.
    pub boundary: Vec<(Id<Edge>, Range<u32>)>,
}

/// One result per face (L6): a face that will not mesh is named, and its neighbours are unaffected (P9).
pub struct Mesh {
    pub edges: Vec<EdgeSamples>,
    pub faces: Vec<Result<FaceMesh, Report>>,
}

/// Every shared edge, one result each, in the order given.
#[must_use]
pub fn sample_edges(
    exec: &Exec,
    model: Arc<Snapshot>,
    edges: Arc<[Id<Edge>]>,
    limits: Limits,
) -> Vec<Result<EdgeSamples, Report>> {
    let count = edges.len();
    exec.map(count, Grain::Each, move |index| unimplemented!())
}

/// Each face against the shared samples, one face a task. Point location uses the exact predicates on the real
/// coordinates, not P9's integer grid, which caused all three of its failures.
#[must_use]
pub fn mesh_faces(
    exec: &Exec,
    model: Arc<Snapshot>,
    samples: Arc<[EdgeSamples]>,
    faces: Arc<[Id<Face>]>,
    limits: Limits,
) -> Vec<Result<FaceMesh, Report>> {
    let count = faces.len();
    exec.map(count, Grain::Each, move |index| unimplemented!())
}

/// Run on every output, in production: every edge used twice in opposite directions, checked and not assumed, because
/// "by construction" covers only the faces that meshed (P9).
#[must_use]
pub fn watertight(mesh: &Mesh) -> Vec<Report> {
    unimplemented!()
}
