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
//! The work runs in parallel only through [`occam_math::Exec`], one face or one edge a task, and every result comes back
//! in id order, so the samples are the same bits at any thread count.

mod curvature;
mod edges;
mod limits;
#[cfg(test)]
mod testing;

pub use edges::{EdgeSamples, Edges, sample_edges};
pub use limits::Limits;
