//! Source: original
//! A model meshed again after an edit, from its mesh before: only the faces the edit touched, and those along an edge
//! whose samples changed, are meshed again, and every other face keeps its block, or the report refusing it, bit for
//! bit. What an edit touched is what its [`Changes`] say it created or changed: every record it wrote, as `Edit` lists
//! them, so an entity in neither list kept its record. The result is the mesh [`crate::mesh`] gives the model whole,
//! checked watertight the same way.
//!
//! A face is touched where anything its mesh is drawn from is: the face, its surface, its loops, their uses and the
//! uses' curves in its parameters, and the uses' edges with their curves and vertices. Every edge a touched face uses
//! is sampled again, its step read again from every face along it, since a face's bending is part of the step it asks.
//! An edge whose faces are all untouched keeps its samples: where a face that used it went, the uses left round it were
//! rewritten, and each is a face's, as the checker lets no edge be both a loop's and a wire's, so that face is touched.
//! An edge sampled again is compared with its old samples to the bit, and every face along one whose samples changed is
//! meshed again with the touched faces.

use std::collections::{BTreeMap, BTreeSet};

use occam_math::Exec;
use occam_report::{EntityRef, Report};
use occam_topo::{Changes, Edge, Face, Id, Loop, Snapshot, View};

use crate::edges::{EdgeSamples, Edges, sample_some};
use crate::faces::{FaceMesh, Faces, mesh_some};
use crate::merge::{Mesh, merge};
use crate::watertight::watertight;

/// A model meshed again from its mesh before.
#[derive(Clone, Debug, PartialEq)]
pub struct Remeshed {
    /// The model's mesh: the one [`crate::mesh`] gives it whole.
    pub mesh: Mesh,
    /// The faces meshed again, in id order. Every other face kept its block, or its refusal, from the mesh before.
    pub meshed: Vec<Id<Face>>,
    /// The edges sampled again, in id order. Every other edge kept its samples, or its refusal.
    pub sampled: Vec<Id<Edge>>,
}

/// `model` meshed on `exec` at the limits `before` was made to, from `before`, the mesh of the model the edits that
/// `changes` describe began from: only the faces they touched, and those along an edge whose samples changed, are
/// meshed again, and the rest keep their blocks and refusals bit for bit. The mesh is the one [`crate::mesh`] gives
/// `model` whole, checked watertight the same way.
#[must_use]
pub fn remesh<'a>(
    exec: &Exec,
    model: &Snapshot,
    before: &Mesh,
    changes: impl IntoIterator<Item = &'a Changes>,
) -> Remeshed {
    let written = changes.into_iter().flat_map(|c| c.created.iter().chain(&c.changed));
    let touched: BTreeSet<EntityRef> = written.copied().collect();
    let faces = model.ids::<Face>();
    let users = users(model, &faces);
    let (mut kept, first) = split(model, before, &faces, &touched);
    let (edges, sampled, moved) = resampled(exec, model, before, &users, &first);
    let along = moved.iter().flat_map(|e| users.get(e).into_iter().flatten().copied());
    let meshed: Vec<Id<Face>> = first.into_iter().chain(along).collect::<BTreeSet<_>>().into_iter().collect();
    let mut again: Meshes = mesh_some(exec, model, &meshed, &edges).into_iter().collect();
    let each = |face: &Id<Face>| again.remove(face).or_else(|| kept.remove(face)).map(|mesh| (*face, mesh));
    let mut mesh = merge(edges, &Faces::new(faces.iter().filter_map(each).collect()));
    mesh.open = watertight(model, &mesh);
    Remeshed { mesh, meshed, sampled }
}

/// Faces with their meshes or the reports refusing them, by id.
type Meshes = BTreeMap<Id<Face>, Result<FaceMesh, Report>>;

/// Each edge the faces of `model` use, in id order, with the faces that use it, in id order: `faces`, all of them.
pub(crate) fn users(model: &Snapshot, faces: &[Id<Face>]) -> BTreeMap<Id<Edge>, Vec<Id<Face>>> {
    let mut users: BTreeMap<Id<Edge>, Vec<Id<Face>>> = BTreeMap::new();
    for &face in faces {
        for edge in model.view(face).map(View::<Face>::edges).unwrap_or_default() {
            users.entry(edge).or_default().push(face);
        }
    }
    users
}

/// `faces` split into those that keep their meshes or refusals from `before`, with them, and those to be meshed again,
/// in id order: those `touched` reaches, and any `before` does not hold.
fn split(
    model: &Snapshot,
    before: &Mesh,
    faces: &[Id<Face>],
    touched: &BTreeSet<EntityRef>,
) -> (Meshes, Vec<Id<Face>>) {
    let (mut kept, mut again) = (BTreeMap::new(), Vec::new());
    for &face in faces {
        match (!drawn_from(model, face, touched)).then(|| before.own(face)).flatten() {
            Some(own) => {
                kept.insert(face, own);
            }
            None => again.push(face),
        }
    }
    (kept, again)
}

/// The edges of `users` sampled again on `exec` where one of `first`, the faces meshed again whatever their edges'
/// samples, uses them, and the rest as `before` sampled them; with those sampled again and those whose samples changed,
/// each in id order.
fn resampled(
    exec: &Exec,
    model: &Snapshot,
    before: &Mesh,
    users: &BTreeMap<Id<Edge>, Vec<Id<Face>>>,
    first: &[Id<Face>],
) -> (Edges, Vec<Id<Edge>>, Vec<Id<Edge>>) {
    let wanted: BTreeSet<Id<Edge>> =
        first.iter().flat_map(|&f| model.view(f).map(View::<Face>::edges).unwrap_or_default()).collect();
    // Every face along an edge sampled again, for its bending, in id order.
    let bent: BTreeSet<Id<Face>> = wanted.iter().flat_map(|e| users.get(e).into_iter().flatten().copied()).collect();
    let bent: Vec<Id<Face>> = bent.into_iter().collect();
    let limits = before.edges.limits();
    let fresh: BTreeMap<Id<Edge>, Result<EdgeSamples, Report>> =
        sample_some(exec, model, &bent, &|e| wanted.contains(&e), limits).into_iter().collect();
    let moved = fresh.iter().filter(|(e, now)| !same(before.edges.get(**e), now.as_ref())).map(|(e, _)| *e).collect();
    let old = |e: Id<Edge>| before.edges.get(e).map(|was| was.cloned().map_err(Report::clone));
    let list = users.keys().filter_map(|&e| fresh.get(&e).cloned().or_else(|| old(e)).map(|samples| (e, samples)));
    (Edges::new(list.collect(), limits), wanted.into_iter().collect(), moved)
}

/// Whether anything the mesh of `face` is drawn from is in `touched`: the face, its surface, its loops, their uses and
/// the uses' curves in its parameters, and the uses' edges with their curves and vertices.
pub(crate) fn drawn_from(model: &Snapshot, face: Id<Face>, touched: &BTreeSet<EntityRef>) -> bool {
    let Some(view) = model.view(face) else { return true };
    let mut parts = vec![face.entity(), view.surface_id().entity()];
    for each in view.loops() {
        parts.push(each.entity());
        let uses = model.view(each).map(View::<Loop>::uses).unwrap_or_default();
        for used in uses.into_iter().filter_map(|u| model.view(u)) {
            parts.push(used.id().entity());
            parts.extend(used.pcurve_id().map(Id::entity));
            parts.extend(edge_parts(model, used.edge()));
        }
    }
    parts.iter().any(|part| touched.contains(part))
}

/// An edge, its curve and its vertices.
fn edge_parts(model: &Snapshot, edge: Id<Edge>) -> Vec<EntityRef> {
    let mut parts = vec![edge.entity()];
    if let Some(view) = model.view(edge) {
        let (start, finish) = view.ends();
        parts.extend([view.curve_id().entity(), start.entity(), finish.entity()]);
    }
    parts
}

/// Whether `now` is what `before` held for an edge, every number to the bit: never for an edge `before` did not
/// sample.
pub(crate) fn same(before: Option<Result<&EdgeSamples, &Report>>, now: Result<&EdgeSamples, &Report>) -> bool {
    before.is_some_and(|was| was.map(bits) == now.map(bits))
}

/// Every number of `samples`, as its bits.
fn bits(samples: &EdgeSamples) -> Vec<u64> {
    let points = samples.points.iter().flat_map(|p| {
        let v = p.to_vec3();
        [v.x, v.y, v.z]
    });
    samples.params.iter().copied().chain(points).map(f64::to_bits).collect()
}

#[cfg(test)]
mod tests;
