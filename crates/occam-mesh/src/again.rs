//! Source: original
//! What a page that drew one model must mesh again to draw the next, read from the two models alone, as `occam view`'s
//! page is given each model the command line saves (docs/architecture/live-view.md). The faces are those [`remesh`]
//! would mesh again after the edits between them: each face drawn from anything written since, read by content
//! through [`Snapshot::written_since`], and each face along an edge one of those uses whose samples changed, the edge
//! sampled in each model as [`crate::mesh`] samples it there; and each face along an edge that one of those faces cuts
//! otherwise in the two models ([`crate::repair`]), as every other face's boundary, and so its cuts, are as they were.
//! Every other face's block, or the report refusing it, is the one the model before gave it, bit for bit, and so is
//! every edge only those faces use, so the page keeps them and what was reported of them.
//!
//! [`remesh`]: crate::remesh

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use occam_math::{Exec, Grain};
use occam_report::{EntityRef, Report};
use occam_topo::{Edge, Face, Id, Snapshot, View};

use crate::edges::{EdgeSamples, Edges, sample_some};
use crate::limits::Limits;
use crate::remesh::{drawn_from, same, users};
use crate::repair::{Cuts, cuts};

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
    // The faces meshed again may cut their edges otherwise than they did: each face along an edge whose cuts differ.
    let listed: Vec<Id<Face>> = again.iter().copied().collect();
    let recut = recut(&cut(exec, before, &listed, limits), &cut(exec, after, &listed, limits));
    let along = recut.iter().flat_map(|edge| users.get(edge).into_iter().flatten().copied());
    let again: BTreeSet<Id<Face>> = again.into_iter().chain(along).collect();
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

/// Each of `faces` with the cuts repair makes in it in `model`, on `exec`, against its edges sampled to `limits` as
/// [`crate::mesh`] samples them there: none for a face `model` does not have.
fn cut(exec: &Exec, model: &Snapshot, faces: &[Id<Face>], limits: Limits) -> BTreeMap<Id<Face>, Cuts> {
    let edges_of = |face: Id<Face>| model.view(face).map(View::<Face>::edges).unwrap_or_default();
    let wanted: BTreeSet<Id<Edge>> = faces.iter().flat_map(|&face| edges_of(face)).collect();
    let edges = Edges::new(sampled(exec, model, &wanted, limits).into_iter().collect(), limits);
    let (read, sampled, of) = (Arc::new(model.clone()), Arc::new(edges), Arc::<[Id<Face>]>::from(faces));
    let each = exec.map(faces.len(), Grain::Each, move |k| of.get(k).map(|&face| cuts(&read, face, &sampled)));
    faces.iter().copied().zip(each.into_iter().map(Option::unwrap_or_default)).collect()
}

/// The edges whose cuts by a face differ between `was` and `now`, in id order.
fn recut(was: &BTreeMap<Id<Face>, Cuts>, now: &BTreeMap<Id<Face>, Cuts>) -> BTreeSet<Id<Edge>> {
    let none = Cuts::new();
    let faces: BTreeSet<&Id<Face>> = was.keys().chain(now.keys()).collect();
    let mut differ = BTreeSet::new();
    for face in faces {
        let (then, here) = (was.get(face).unwrap_or(&none), now.get(face).unwrap_or(&none));
        let edges: BTreeSet<&Id<Edge>> = then.keys().chain(here.keys()).collect();
        differ.extend(edges.into_iter().filter(|edge| then.get(*edge) != here.get(*edge)).copied());
    }
    differ
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
