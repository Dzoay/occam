//! Source: original
//! What a page that drew one model must mesh again to draw the next, read from the two models alone, as `occam view`'s
//! page is given each model the command line saves (docs/architecture/live-view.md). The faces are those [`remesh`]
//! would mesh again after the edits between them: each face drawn from anything written since, read by content
//! through [`Snapshot::written_since`], and each face along an edge one of those uses whose samples changed, the edge
//! sampled in each model as [`crate::mesh`] samples it there. Every other face's block, or the report refusing it, is
//! the one the model before gave it, bit for bit, and so is every edge only those faces use, so the page keeps them
//! and what was reported of them.
//!
//! [`remesh`]: crate::remesh

use std::collections::{BTreeMap, BTreeSet};

use occam_math::Exec;
use occam_report::{EntityRef, Report};
use occam_topo::{Edge, Face, Id, Snapshot, View};

use crate::edges::{EdgeSamples, sample_some};
use crate::limits::Limits;
use crate::remesh::{drawn_from, same, users};

/// What a page that drew a model must mesh again to draw the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Again {
    /// The faces to mesh again, in id order.
    pub faces: Vec<Id<Face>>,
    /// What keeps its mesh from the model before: every other face, in id order, then each edge only those faces use,
    /// in id order. What the mesh before reported of these still holds; what it reported of anything else does not.
    pub kept: Vec<EntityRef>,
}

/// What a page that drew `before` meshed to `limits` must mesh again, on `exec`, to draw `after` meshed to them: the
/// faces [`crate::remesh`] would mesh again after the edits between them, and what keeps its mesh.
#[must_use]
pub fn again(exec: &Exec, before: &Snapshot, after: &Snapshot, limits: Limits) -> Again {
    let touched: BTreeSet<EntityRef> = after.written_since(before).into_iter().collect();
    let faces = after.ids::<Face>();
    let users = users(after, &faces);
    let first: Vec<Id<Face>> = faces.iter().copied().filter(|&face| drawn_from(after, face, &touched)).collect();
    let edges_of = |face: Id<Face>| after.view(face).map(View::<Face>::edges).unwrap_or_default();
    let wanted: BTreeSet<Id<Edge>> = first.iter().flat_map(|&face| edges_of(face)).collect();
    let moved = moved(&sampled(exec, before, &wanted, limits), &sampled(exec, after, &wanted, limits));
    let along = moved.iter().flat_map(|edge| users.get(edge).into_iter().flatten().copied());
    let again: BTreeSet<Id<Face>> = first.into_iter().chain(along).collect();
    let kept_faces = faces.iter().filter(|face| !again.contains(face)).map(|face| face.entity());
    let only_kept = |faces: &Vec<Id<Face>>| faces.iter().all(|face| !again.contains(face));
    let kept_edges = users.iter().filter(|(_, faces)| only_kept(faces)).map(|(edge, _)| edge.entity());
    let kept = kept_faces.chain(kept_edges).collect();
    Again { faces: again.into_iter().collect(), kept }
}

/// Edges with their samples or the reports refusing them, by id.
type Samples = BTreeMap<Id<Edge>, Result<EdgeSamples, Report>>;

/// The edges of `model` in `wanted` that a face uses, each sampled on `exec` to `limits` as [`crate::mesh`] samples it
/// there, its step read from every face along it, by id.
fn sampled(exec: &Exec, model: &Snapshot, wanted: &BTreeSet<Id<Edge>>, limits: Limits) -> Samples {
    let faces_of = |edge: Id<Edge>| model.view(edge).map(View::<Edge>::faces).unwrap_or_default();
    let bent: BTreeSet<Id<Face>> = wanted.iter().flat_map(|&edge| faces_of(edge)).collect();
    let bent: Vec<Id<Face>> = bent.into_iter().collect();
    sample_some(exec, model, &bent, &|edge| wanted.contains(&edge), limits).into_iter().collect()
}

/// The edges of `now` whose samples, or refusal, are not what `was` holds for them to the bit, in id order: an edge
/// `was` does not hold among them.
fn moved(was: &Samples, now: &Samples) -> Vec<Id<Edge>> {
    let differs = |(edge, samples): &(&Id<Edge>, &Result<EdgeSamples, Report>)| {
        !same(was.get(*edge).map(Result::as_ref), samples.as_ref())
    };
    now.iter().filter(differs).map(|(edge, _)| *edge).collect()
}

#[cfg(test)]
mod tests;
