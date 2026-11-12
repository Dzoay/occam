//! Source: original
//! What a commit checks (P1): everything the edit created, changed or removed that is live after it, and every live
//! neighbour whose own checks read one of those, found both in the snapshot the edit began from and in the new model.
//! Neighbours are what a record links to and the short lists and rings it sits in: its owner's list, the uses round an
//! edge, the edges round a vertex; and the face of every loop reached, whose loops' nesting the loop's edges decide.
//! Four edits reach further, to everything they could affect: moving a shell to another body, changing a body's kind,
//! removing a shell, and removing or replacing in place a curve or surface, which reaches what lies on it and its
//! neighbours.

use std::collections::BTreeSet;

use occam_report::{EntityKind, EntityRef};

use super::read::{Walk, first_use, get, list, note, radial, star, typed, uses_from};
use super::shell::{face_edges, faces};
use crate::edit::Edit;
use crate::kind::sealed::Sealed;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Kind, Loop, Shell, Vertex, Wire};
use crate::records::{Around, End, UseOwner};
use crate::snapshot::Model;

/// The live entities a commit of `edit` checks, in kind order, then slot order.
pub(crate) fn scope(edit: &Edit) -> BTreeSet<EntityRef> {
    let (base, next) = (&edit.base.model, &edit.next);
    let mut found = BTreeSet::new();
    for &entity in edit.created.iter().chain(&edit.changed).chain(&edit.deleted) {
        found.insert(entity);
        around(base, entity, &mut found);
        around(next, entity, &mut found);
        further(base, next, entity, &mut found);
    }
    let faces: Vec<EntityRef> =
        found.iter().filter_map(|&l| typed::<Loop>(l)).filter_map(|l| get(next, l)).map(|l| l.face.entity()).collect();
    found.extend(faces);
    found.retain(|&entity| next.is_live(entity));
    found
}

/// Adds each item of `walk`.
fn add<K: Kind>(out: &mut BTreeSet<EntityRef>, walk: &Walk<Id<K>>) {
    out.extend(walk.items().into_iter().map(Id::entity));
}

/// Adds the uses of a loop or wire from `first`, and their edges.
fn add_uses(model: &Model, first: Id<EdgeUse>, out: &mut BTreeSet<EntityRef>) {
    for u in uses_from(model, first).items() {
        out.insert(u.entity());
        out.extend(get(model, u).map(|r| r.edge.entity()));
    }
}

/// Adds what `entity` links to in `model`, and the lists and rings it sits in there.
fn around(model: &Model, entity: EntityRef, out: &mut BTreeSet<EntityRef>) {
    let mut links = Vec::new();
    note();
    model.links(entity, &mut links);
    out.extend(links);
    match entity.kind {
        EntityKind::EdgeUse => typed::<EdgeUse>(entity).into_iter().for_each(|id| use_around(model, id, out)),
        EntityKind::Edge => typed::<Edge>(entity).into_iter().for_each(|id| edge_around(model, id, out)),
        EntityKind::Vertex => {
            let start = typed::<Vertex>(entity).and_then(|id| get(model, id)).and_then(|v| v.star);
            out.extend(start.into_iter().flat_map(|s| star(model, s).items()).map(|p| p.edge.entity()));
        }
        EntityKind::Loop => typed::<Loop>(entity).into_iter().for_each(|id| loop_around(model, id, out)),
        EntityKind::Face => {
            out.extend(typed::<Face>(entity).into_iter().flat_map(|id| face_edges(model, id)).map(Id::entity));
            let outer = typed::<Face>(entity).and_then(|id| get(model, id)).map(|f| f.outer);
            add(out, &list(model, outer, |l| l.next));
        }
        EntityKind::Shell => {
            let body = typed::<Shell>(entity).and_then(|id| get(model, id)).map(|s| s.body);
            add(out, &list(model, body.and_then(|b| get(model, b)).and_then(|b| b.first_shell), |s| s.next));
        }
        EntityKind::Wire => typed::<Wire>(entity).into_iter().for_each(|id| wire_around(model, id, out)),
        EntityKind::Body => {
            let record = typed::<Body>(entity).and_then(|id| get(model, id));
            add(out, &list(model, record.and_then(|b| b.first_shell), |s| s.next));
            add(out, &list(model, record.and_then(|b| b.first_wire), |w| w.next));
        }
        EntityKind::Surface
        | EntityKind::Curve
        | EntityKind::ParamCurve
        | EntityKind::Model
        | EntityKind::Section
        | EntityKind::Region => {}
    }
}

/// A use's owner's list and its edge's ring.
fn use_around(model: &Model, id: Id<EdgeUse>, out: &mut BTreeSet<EntityRef>) {
    let Some(record) = get(model, id) else { return };
    if let Some(first) = first_use(model, record.owner) {
        add(out, &uses_from(model, first));
    }
    if let Some(edge) = get(model, record.edge) {
        add(out, &radial(model, edge));
    }
}

/// An edge's ring of uses and their owners, and the stars at its ends.
fn edge_around(model: &Model, id: Id<Edge>, out: &mut BTreeSet<EntityRef>) {
    let Some(record) = get(model, id) else { return };
    for u in radial(model, record).items() {
        out.insert(u.entity());
        out.extend(get(model, u).map(|r| match r.owner {
            UseOwner::Loop(l) => l.entity(),
            UseOwner::Wire(w) => w.entity(),
        }));
    }
    for end in [End::Start, End::Finish] {
        out.extend(star(model, Around { edge: id, end }).items().into_iter().map(|p| p.edge.entity()));
    }
}

/// A loop's face's list of loops, and its uses and their edges.
fn loop_around(model: &Model, id: Id<Loop>, out: &mut BTreeSet<EntityRef>) {
    let Some(record) = get(model, id) else { return };
    add(out, &list(model, get(model, record.face).map(|f| f.outer), |l| l.next));
    add_uses(model, record.first, out);
}

/// A wire's body's list of wires, and its uses and their edges.
fn wire_around(model: &Model, id: Id<Wire>, out: &mut BTreeSet<EntityRef>) {
    let Some(record) = get(model, id) else { return };
    add(out, &list(model, get(model, record.body).and_then(|b| b.first_wire), |w| w.next));
    add_uses(model, record.first, out);
}

/// The edges of every face of a shell, and its faces.
fn shell_edges(model: &Model, id: Id<Shell>, out: &mut BTreeSet<EntityRef>) {
    for face in faces(model, id) {
        out.insert(face.entity());
        out.extend(face_edges(model, face).into_iter().map(Id::entity));
    }
}

/// Adds what the four far-reaching edits can affect.
fn further(base: &Model, next: &Model, entity: EntityRef, out: &mut BTreeSet<EntityRef>) {
    match entity.kind {
        EntityKind::Shell => {
            let Some(id) = typed::<Shell>(entity) else { return };
            let (was, is) = (get(base, id).map(|s| s.body), get(next, id).map(|s| s.body));
            if is.is_none() {
                shell_edges(base, id, out);
            } else if was.is_some() && was != is {
                shell_edges(next, id, out);
            }
        }
        EntityKind::Body => {
            let Some(id) = typed::<Body>(entity) else { return };
            let (was, is) = (get(base, id).map(|b| b.kind), get(next, id));
            if let (Some(was), Some(record)) = (was, is)
                && was != record.kind
            {
                for shell in list(next, record.first_shell, |s| s.next).items() {
                    shell_edges(next, shell, out);
                }
                for wire in list(next, record.first_wire, |w| w.next).items() {
                    wire_around(next, wire, out);
                }
            }
        }
        EntityKind::Surface | EntityKind::Curve | EntityKind::ParamCurve if base.is_live(entity) => {
            let mut users = BTreeSet::new();
            lying_on(next, entity, &mut users);
            for &user in &users {
                around(next, user, out);
            }
            out.extend(users);
        }
        _ => {}
    }
}

/// Adds everything in `model` that names the geometry `entity`, reading, and counting, every face, edge and use.
fn lying_on(model: &Model, entity: EntityRef, out: &mut BTreeSet<EntityRef>) {
    let faces = Face::table(model).records().inspect(|_| note()).filter(|(_, _, r)| r.surface.entity() == entity);
    out.extend(faces.map(|(slot, generation, _)| Id::<Face>::new(slot, generation).entity()));
    let edges = Edge::table(model).records().inspect(|_| note()).filter(|(_, _, r)| r.curve.entity() == entity);
    out.extend(edges.map(|(slot, generation, _)| Id::<Edge>::new(slot, generation).entity()));
    let uses = EdgeUse::table(model).records().inspect(|_| note());
    let uses = uses.filter(|(_, _, r)| r.pcurve.map(Id::entity) == Some(entity));
    out.extend(uses.map(|(slot, generation, _)| Id::<EdgeUse>::new(slot, generation).entity()));
}
