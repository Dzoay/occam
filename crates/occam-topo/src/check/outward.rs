//! Source: original
//! Which solids a check asks whether they face out, the last question it asks, and only of a model that passes every
//! other check: the answer reads the loops' directions, the faces' normals and the shells' closure, which the other
//! checks vouch for, and [`crate::geometric::inside_out`] gives it from the whole body.
//!
//! The whole-model check asks every solid. A commit asks each solid the edit may have turned round at every face, which
//! a walk from each of its shells' first faces through those faces finds, stopping at the first face the edit could not
//! have turned: so the cost follows the edit (P1), and a solid made, moved or mirrored whole, or every face of it turned
//! round, is asked. It also asks each body the edit made a solid, and each body a shell moved to. A solid that keeps a
//! face as it was keeps that face's orientation, which its neighbours' must agree with across every edge (the loops'
//! winding and the edges' opposed uses): it could be turned inside out only by rebuilding the rest of it on that face's
//! other side, which the whole-model check, as loading runs it, still finds.

use std::collections::BTreeSet;

use occam_report::{EntityKind, EntityRef, Report};

use super::read::{get, note, typed};
use super::shell::{edge_faces, face_edges};
use crate::edit::Edit;
use crate::kind::sealed::Sealed;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Shell};
use crate::records::{BodyKind, UseOwner};
use crate::snapshot::Model;

/// Reports each solid of `model` that faces in, in slot order.
pub(crate) fn whole(model: &Model, out: &mut Vec<Report>) {
    let solids = Body::table(model).records().filter(|(_, _, r)| r.kind == BodyKind::Solid);
    let ids: Vec<Id<Body>> = solids.map(|(slot, generation, _)| Id::new(slot, generation)).collect();
    out.extend(ids.into_iter().filter_map(|b| crate::geometric::inside_out(model, b)));
}

/// Reports each solid `edit` rebuilt whole, made a solid or moved a shell to, that faces in, in id order.
pub(crate) fn edit(edit: &Edit, out: &mut Vec<Report>) {
    let next = &edit.next;
    let rebuilt = rebuilt(edit);
    let mut asked: BTreeSet<Id<Body>> = rebuilt.iter().filter_map(|&f| face_body(next, f)).collect();
    asked.retain(|&b| whole_rebuilt(next, b, &rebuilt));
    asked.extend(edit.changed.iter().filter_map(|&entity| made_solid(edit, entity)));
    let solid = |&b: &Id<Body>| get(next, b).is_some_and(|r| r.kind == BodyKind::Solid);
    out.extend(asked.into_iter().filter(solid).filter_map(|b| crate::geometric::inside_out(next, b)));
}

/// The faces of `edit`'s model whose way round it may have changed: those with a use it made or changed, an edge it made
/// or changed, or an edge on a curve it replaced in place, which only such an edit pays a reading of every edge to find.
/// A face faces the way its loops run about it, which only its uses and their edges' curves say: the loops' winding in
/// its surface's parameters must agree with it, and its other records, its surface included, cannot turn it round.
fn rebuilt(edit: &Edit) -> BTreeSet<Id<Face>> {
    let next = &edit.next;
    let touched: Vec<EntityRef> = edit.created.iter().chain(&edit.changed).copied().collect();
    let mut edges: BTreeSet<Id<Edge>> = touched.iter().filter_map(|&e| typed::<Edge>(e)).collect();
    let curves: BTreeSet<EntityRef> = edit.changed.iter().filter(|e| e.kind == EntityKind::Curve).copied().collect();
    if !curves.is_empty() {
        let on = Edge::table(next).records().inspect(|_| note()).filter(|(_, _, r)| curves.contains(&r.curve.entity()));
        edges.extend(on.map(|(slot, generation, _)| Id::new(slot, generation)));
    }
    let used = touched.iter().filter_map(|&u| typed::<EdgeUse>(u)).filter_map(|u| use_face(next, u));
    used.chain(edges.into_iter().flat_map(|e| edge_faces(next, e))).collect()
}

/// The face of use `id`'s loop, if a loop owns it.
fn use_face(model: &Model, id: Id<EdgeUse>) -> Option<Id<Face>> {
    match get(model, id)?.owner {
        UseOwner::Loop(l) => get(model, l).map(|l| l.face),
        UseOwner::Wire(_) => None,
    }
}

/// The body of face `id`'s shell.
fn face_body(model: &Model, id: Id<Face>) -> Option<Id<Body>> {
    get(model, get(model, id)?.shell).map(|s| s.body)
}

/// Whether every face of body `id` is in `rebuilt`: a walk from each shell's first face through shared edges, kept to
/// faces in `rebuilt`, that stops at the first face not in it.
fn whole_rebuilt(model: &Model, id: Id<Body>, rebuilt: &BTreeSet<Id<Face>>) -> bool {
    let mut shells = Vec::new();
    let mut next = get(model, id).and_then(|b| b.first_shell);
    // A sound list of shells ends before it has named more than the table has slots.
    while let Some(shell) = next.filter(|_| shells.len() <= Shell::table(model).len()) {
        shells.push(shell);
        next = get(model, shell).and_then(|s| s.next);
    }
    shells.into_iter().all(|shell| {
        let Some(start) = get(model, shell).map(|s| s.face) else { return false };
        let (mut seen, mut frontier) = (BTreeSet::from([start]), vec![start]);
        while let Some(face) = frontier.pop() {
            if !rebuilt.contains(&face) {
                return false;
            }
            let near = face_edges(model, face).into_iter().flat_map(|e| edge_faces(model, e));
            frontier.extend(near.filter(|&f| seen.insert(f)).collect::<Vec<_>>());
        }
        true
    })
}

/// The body `entity` made a solid, if it is a body that was not one, or the body it moved to, if it is a shell that
/// moved.
fn made_solid(edit: &Edit, entity: EntityRef) -> Option<Id<Body>> {
    let (base, next) = (&edit.base.model, &edit.next);
    match entity.kind {
        EntityKind::Body => {
            let id = typed::<Body>(entity)?;
            (get(base, id)?.kind != BodyKind::Solid).then_some(id)
        }
        EntityKind::Shell => {
            let id = typed::<Shell>(entity)?;
            let (was, is) = (get(base, id)?.body, get(next, id)?.body);
            (was != is).then_some(is)
        }
        _ => None,
    }
}
