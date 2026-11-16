//! Source: original
//! Repair (P9), where a face's boundary, drawn through its edges' samples in its parameters, crosses or touches itself,
//! or a loop joins up to fewer than three points, and finer samples can part it: two boundaries closer than the samples'
//! spacing, as where a coarse chord cuts across a hole under it. Repair names the edges that collided, those whose
//! pieces the failure runs along, and cuts each of those pieces at its middle, never another edge: P9 cut every edge of
//! a face that failed finer, and one model went from 1 s to 144 s. The boundary is tried again, and cut again, up to
//! [`Limits::REPAIRS_PER_FACE`] times, and never past [`Limits::SAMPLES_PER_EDGE`] on any edge.
//!
//! Finer samples cannot part two points at one place, a decision beyond what the coordinates allow, or a segment that
//! is no piece of an edge, as where a loop crosses a pole from one side of its face to the other: repair is not tried
//! there. A face it does not cure keeps no cut and is named, as [`crate::mesh_face`] names it, with the edges that
//! collided.
//!
//! A face's cuts depend only on the face and its edges' first samples. Each edge takes the cuts of every face along it,
//! and each face along an edge cut is meshed again; so a mesh, a part of one, and a mesh made again after an edit all
//! cut each edge alike.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use occam_math::{Exec, Grain};
use occam_report::{Code, Reason, Report};
use occam_topo::{Edge, Face, Id, Snapshot, View};

use crate::boundary::{self, Piece};
use crate::edges::Edges;
use crate::faces::{FaceMesh, mesh_some, plane};
use crate::limits::Limits;
use crate::triangulation::Triangulation;

/// Where repair cuts each edge: parameters strictly between two of its samples.
pub(crate) type Cuts = BTreeMap<Id<Edge>, Vec<f64>>;

/// The cuts that cure the boundary of `face` against `edges`: none where it needs none, and none where finer samples
/// cannot cure it within [`Limits::REPAIRS_PER_FACE`] tries and [`Limits::SAMPLES_PER_EDGE`] samples an edge.
pub(crate) fn cuts(model: &Snapshot, face: Id<Face>, edges: &Edges) -> Cuts {
    let mut cuts = Cuts::new();
    for _ in 0..Limits::REPAIRS_PER_FACE {
        let Some(pieces) = collided(model, face, &local(model, face, edges).cut(model, &cuts)) else { return cuts };
        if pieces.is_empty() || !cut(&mut cuts, &pieces, edges) {
            break;
        }
    }
    Cuts::new()
}

/// The edges of `face`, as `edges` holds them.
fn local(model: &Snapshot, face: Id<Face>, edges: &Edges) -> Edges {
    let own: BTreeSet<Id<Edge>> = model.view(face).map(View::<Face>::edges).unwrap_or_default().into_iter().collect();
    let held = |edge: Id<Edge>| edges.get(edge).map(|sampled| (edge, sampled.cloned().map_err(Report::clone)));
    Edges::new(own.into_iter().filter_map(held).collect(), edges.limits())
}

/// The pieces of the boundary of `face` against `edges` that collided, as [`crate::mesh_face`] draws and triangulates
/// it: `None` where they do not, and none where finer samples cannot part them.
fn collided(model: &Snapshot, face: Id<Face>, edges: &Edges) -> Option<Vec<Piece>> {
    let Some((view, surface)) = model.view(face).and_then(|view| Some((view, view.surface()?))) else {
        return Some(Vec::new());
    };
    let Ok(boundary) = boundary::of(model, view, edges) else { return Some(Vec::new()) };
    if let Some(pieces) = boundary.short_pieces() {
        return Some(pieces);
    }
    let (plane, _) = plane(surface, &boundary);
    Triangulation::new(&plane, &boundary.segments).err().map(|fail| boundary.collided(fail))
}

/// Adds to `cuts` the middle of each of `pieces`; `false` where one has no middle strictly inside it, or an edge would
/// have more samples than [`Limits::SAMPLES_PER_EDGE`] over those `edges` gives it.
fn cut(cuts: &mut Cuts, pieces: &[Piece], edges: &Edges) -> bool {
    let cap = usize::try_from(Limits::SAMPLES_PER_EDGE).unwrap_or(usize::MAX);
    for piece in pieces {
        let middle = 0.5 * piece.from + 0.5 * piece.to;
        let Some(Ok(samples)) = edges.get(piece.edge) else { return false };
        if !(piece.from < middle && middle < piece.to) {
            return false;
        }
        let at = cuts.entry(piece.edge).or_default();
        if let Err(k) = at.binary_search_by(|t| t.total_cmp(&middle)) {
            at.insert(k, middle);
        }
        if samples.params.len().saturating_add(at.len()) > cap {
            return false;
        }
    }
    true
}

/// `edges` with the cuts of each of `faces`, each found against `edges`, one face a task on `exec`.
pub(crate) fn mended(exec: &Exec, model: &Snapshot, edges: &Edges, faces: &[Id<Face>]) -> Edges {
    let (read, sampled) = (Arc::new(model.clone()), Arc::new(edges.clone()));
    let of: Arc<[Id<Face>]> = faces.into();
    let each = exec.map(faces.len(), Grain::Each, move |k| of.get(k).map(|&f| cuts(&read, f, &sampled)));
    let mut all = Cuts::new();
    for (edge, at) in each.into_iter().flatten().flatten() {
        all.entry(edge).or_default().extend(at);
    }
    edges.cut(model, &all)
}

/// Whether `meshed` is a refusal that repair may cure: a boundary that crosses itself.
fn crossed(meshed: &Result<FaceMesh, Report>) -> bool {
    meshed
        .as_ref()
        .is_err_and(|report| report.code == Code::FaceNotMeshed && report.reason == Some(Reason::BoundaryCrosses))
}

/// Faces with their meshes or the reports refusing them, in the order given.
pub(crate) type Meshed = Vec<(Id<Face>, Result<FaceMesh, Report>)>;

/// `faces` meshed against `edges` on `exec`, and repaired: the edges are cut where those of `faces` whose boundaries
/// crossed, and `others`, faces along their edges that are not meshed here, collided, and each of `faces` along an edge
/// cut is meshed again. `edges` must hold every edge that `faces` and `others` use, each as [`crate::sample_edges`]
/// samples it; and `others` must be every face that uses an edge of `faces` and is not one of them, for those edges to
/// be cut as the whole model cuts them.
pub(crate) fn repaired(
    exec: &Exec,
    model: &Snapshot,
    faces: &[Id<Face>],
    edges: Edges,
    others: &[Id<Face>],
) -> (Edges, Meshed) {
    let mut meshed = mesh_some(exec, model, faces, &edges);
    let collided: Vec<Id<Face>> = meshed
        .iter()
        .filter(|(_, result)| crossed(result))
        .map(|(face, _)| *face)
        .chain(others.iter().copied())
        .collect();
    if collided.is_empty() {
        return (edges, meshed);
    }
    let edges = mended(exec, model, &edges, &collided);
    let cut: BTreeSet<Id<Edge>> = edges.repaired().iter().copied().collect();
    let along = |face: &Id<Face>| model.view(*face).is_some_and(|view| view.edges().iter().any(|e| cut.contains(e)));
    let again: Vec<Id<Face>> = faces.iter().filter(|face| along(face)).copied().collect();
    let mut fresh: BTreeMap<Id<Face>, Result<FaceMesh, Report>> =
        mesh_some(exec, model, &again, &edges).into_iter().collect();
    for (face, result) in &mut meshed {
        if let Some(again) = fresh.remove(face) {
            *result = again;
        }
    }
    (edges, meshed)
}

#[cfg(test)]
mod tests;
