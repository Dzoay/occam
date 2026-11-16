//! Source: original
//! Some of a model's faces meshed alone, as a mesh worker in the browser meshes a range of them
//! (docs/architecture/browser.md). Each edge they use is sampled as [`crate::mesh`] samples it, its step read from every
//! face along it, in the part or not, and cut as repair cuts it there: the boundary of every face along it outside the
//! part is checked too, as one that collided cuts it. So each face's block is the whole mesh's bit for bit, and the
//! parts of a model's faces, laid end to end in id order, are the buffers its whole mesh gives. Each part is checked
//! watertight as far as it reaches.

use std::collections::BTreeSet;

use occam_math::Exec;
use occam_topo::{Edge, Face, Id, Snapshot, View};

use crate::edges::{Edges, sample_some};
use crate::faces::Faces;
use crate::limits::Limits;
use crate::merge::{Mesh, merge};
use crate::repair::repaired;
use crate::watertight::within;

/// `faces` of `model`, taken in id order, meshed on `exec` to `limits` and merged into a mesh of their own, numbered
/// from its start: the edges they use, each sampled as [`crate::mesh`] samples it, their blocks, which hold the bits
/// the whole mesh's blocks of those faces hold, the reports refusing any of them, and what checking the part
/// watertight as far as it reaches found. A segment along an edge that a face outside the part uses may be used once,
/// a face outside the part is no hole, and only a shell whose faces are all in the part has its Euler characteristic
/// counted. Every face of `model`, as one part, gives the mesh [`crate::mesh`] gives; the same bits at any thread count.
#[must_use]
pub fn mesh_part(exec: &Exec, model: &Snapshot, limits: Limits, faces: &[Id<Face>]) -> Mesh {
    let part: BTreeSet<Id<Face>> = faces.iter().copied().collect();
    let edges_of = |face: Id<Face>| model.view(face).map_or_default(View::<Face>::edges);
    let faces_of = |edge: Id<Edge>| model.view(edge).map_or_default(View::<Edge>::faces);
    let wanted: BTreeSet<Id<Edge>> = part.iter().flat_map(|&face| edges_of(face)).collect();
    // The faces outside the part along a wanted edge, whose boundaries repair checks, as they may cut it.
    let ring: BTreeSet<Id<Face>> =
        wanted.iter().flat_map(|&edge| faces_of(edge)).filter(|f| !part.contains(f)).collect();
    let reach: BTreeSet<Id<Edge>> =
        wanted.iter().copied().chain(ring.iter().flat_map(|&face| edges_of(face))).collect();
    // Every face along an edge sampled, in id order, for its bending: the step `sample_edges` gives the edge.
    let bent: BTreeSet<Id<Face>> = reach.iter().flat_map(|&edge| faces_of(edge)).collect();
    let bent: Vec<Id<Face>> = bent.into_iter().collect();
    let edges = Edges::new(sample_some(exec, model, &bent, &|edge| reach.contains(&edge), limits), limits);
    let (ordered, ring): (Vec<Id<Face>>, Vec<Id<Face>>) = (part.iter().copied().collect(), ring.into_iter().collect());
    let (edges, meshed) = repaired(exec, model, &ordered, edges, &ring);
    let mut merged = merge(edges.only(&|edge| wanted.contains(&edge)), &Faces::new(meshed));
    merged.open = within(model, &merged, &|face| part.contains(&face));
    merged
}

#[cfg(test)]
mod tests;
