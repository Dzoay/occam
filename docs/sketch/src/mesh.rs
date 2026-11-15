//! `occam-mesh`: P9's method. Sample each shared edge once, mesh each face against those samples, one face a task,
//! merge by index; 4.7 to 5.2 times at six threads with the same bits at every thread count and lane width. The limits,
//! the edges' samples and the faces' meshes are real (`view.edges`, `view.faces`): what is left is the merge and the
//! check.

use occam_mesh::{Edges, Faces};
use occam_report::Report;

/// The edges' samples and the faces' meshes, merged by index into one vertex block per face (P12's buffer layout).
pub struct Mesh {
    pub edges: Edges,
    pub faces: Faces,
}

/// Run on every output, in production: every edge used twice in opposite directions, checked and not assumed, because
/// "by construction" covers only the faces that meshed (P9).
#[must_use]
pub fn watertight(mesh: &Mesh) -> Vec<Report> {
    unimplemented!()
}
