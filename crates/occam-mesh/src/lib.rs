//! Source: original
//! Tessellation for display (PARITY 1.7), by P9's method: every edge any face uses is sampled once, as finely as the
//! finest of its faces asks, so that neighbouring faces meet on the same points; each face is then meshed against those
//! samples, one face a task; and every output is checked watertight.
//!
//! The [`Limits`] come first. Without them one collapsed row of a NURBS patch took 11.9 GB in seconds (P9): its noise
//! read as a curvature near 1e15, and the shared edge paid for it. So no surface may ask an edge for a step below the
//! floor, no size estimate is read where the surface collapses, and an edge that would need more than
//! [`Limits::SAMPLES_PER_EDGE`] samples is refused by name, with a deflection at which it fits.
//!
//! Each face is then meshed against [`Edges`] by [`mesh_faces`]: its boundary drawn in its parameters through its
//! edges' samples, triangulated with exact decisions on the coordinates as they are, not P9's integer grid, and
//! refined until it is within the deflection measured to the triangle ([`deviation`] measures it), with a floor for
//! conical points and a cap of [`Limits::TRIANGLES_PER_FACE`]. A face that will not mesh is refused by name, with its
//! reason, and its neighbours are meshed as if it were not there.
//!
//! [`mesh`] does all of it: [`merge`] merges the faces by index into one [`Mesh`], each face's vertices a [`Block`] of
//! its own, welding nothing, and [`watertight`] checks every output, in release builds too, matching vertices by
//! position: every segment used once each way, a face that did not mesh named as a hole, and each shell's mesh with its
//! faces' Euler characteristic. [`Mesh::digest`] is what `gates determinism` holds to its record.
//!
//! [`Buffers`] are a mesh as the renderer contract wants it: flat arrays, each face a vertex block of its own whose
//! indices count from its start. After an edit, [`remesh`] meshes again only the faces the edit touched and those along
//! an edge whose samples changed; the rest keep their blocks bit for bit. [`mesh_part`] meshes some of a model's faces
//! alone, as a mesh worker in the browser meshes a range of them: each block the whole mesh's bit for bit, and the part
//! checked watertight as far as it reaches.
//!
//! The work runs in parallel only through [`occam_math::Exec`], one face or one edge a task, and every result comes back
//! in id order, so the samples and the meshes are the same bits at any thread count.

mod boundary;
mod buffers;
mod curvature;
mod deviation;
mod edges;
mod faces;
mod limits;
mod merge;
mod part;
mod predicates;
#[cfg(any(test, feature = "determinism"))]
#[doc(hidden)]
pub mod probe;
mod refine;
mod remesh;
#[cfg(test)]
mod testing;
/// The tests' models, built outside the tests only for the determinism probe, which nothing that ships turns on.
#[cfg(all(feature = "determinism", not(test)))]
#[allow(dead_code, clippy::indexing_slicing, clippy::unwrap_used, reason = "the tests' models, as they are written")]
mod testing;
mod triangulation;
mod watertight;

pub use buffers::{Buffers, FaceRange};
pub use deviation::deviation;
pub use edges::{EdgeSamples, Edges, sample_edges};
pub use faces::{FaceMesh, Faces, Side, mesh_face, mesh_faces};
pub use limits::Limits;
pub use merge::{Block, Mesh, merge, mesh};
pub use part::mesh_part;
pub use remesh::{Remeshed, remesh};
pub use watertight::watertight;
