//! Source: original
//! What a commit checks (P1): everything the edit created, changed or removed that is live after it, and every live
//! neighbour whose own checks read one of those, found both in the snapshot the edit began from and in the new model.
//! Neighbours are what a record links to and the short lists and rings it sits in: its owner's list, the uses round an
//! edge, the edges round a vertex; and the face of every loop reached, whose loops' nesting the loop's edges decide.
//! Four edits reach further, to everything they could affect: moving a shell to another body, changing a body's kind,
//! removing a shell, and removing or replacing in place a curve or surface, which reaches what lies on it and its
//! neighbours. Each list or ring is walked once in each model however many of its members the edit touched, and what
//! lies on the curves and surfaces is found in one scan however many the edit took away, so the scope follows the edit.

use std::collections::BTreeSet;

use occam_report::{EntityKind, EntityRef};

use super::read::{Walk, first_use, get, list, note, radial, star, typed, uses_from};
use super::shell::{face_edges, faces};
use crate::edit::Edit;
use crate::kind::sealed::Sealed;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Kind, Loop, Shell, Vertex, Wire};
use crate::records::{Around, End, UseOwner};
use crate::snapshot::Model;

/// The lists and rings of one model already added: each list by the entity that holds it and the kind it lists, and
/// each place round a star that came round, so that each is walked once however many of its members an edit touched.
#[derive(Default)]
struct Added {
    /// Each list added, by its holder and the kind of its items.
    lists: BTreeSet<(EntityRef, EntityKind)>,
    /// Each place of a star added that came round, from which any walk gives the same places.
    places: BTreeSet<Around>,
}

impl Added {
    /// Adds each item of the list `holder` holds of kind `K`, walked by `walk`, unless it is added already.
    fn list<K: Kind>(&mut self, out: &mut BTreeSet<EntityRef>, holder: EntityRef, walk: impl FnOnce() -> Walk<Id<K>>) {
        if self.lists.insert((holder, K::ENTITY)) {
            add(out, walk());
        }
    }

    /// Adds the edges round the star from `start`, unless a star that came round has added them already.
    fn star(&mut self, model: &Model, start: Around, out: &mut BTreeSet<EntityRef>) {
        if self.places.contains(&start) {
            return;
        }
        let places = star(model, start);
        if let Walk::Ring(ring) = &places {
            self.places.extend(ring.iter().copied());
        }
        out.extend(places.items().into_iter().map(|p| p.edge.entity()));
    }
}

/// The live entities a commit of `edit` checks, in kind order, then slot order.
pub(crate) fn scope(edit: &Edit) -> BTreeSet<EntityRef> {
    let (base, next) = (&edit.base.model, &edit.next);
    let touched = || edit.created.iter().chain(&edit.changed).chain(&edit.deleted).copied();
    let mut found = BTreeSet::new();
    let (mut was, mut is) = (Added::default(), Added::default());
    for entity in touched() {
        found.insert(entity);
        around(base, entity, &mut found, &mut was);
        around(next, entity, &mut found, &mut is);
        further(base, next, entity, &mut found, &mut is);
    }
    let replaced: BTreeSet<EntityRef> =
        touched().filter(|&entity| geometry(entity.kind) && base.is_live(entity)).collect();
    if !replaced.is_empty() {
        let users = lying_on(next, &replaced);
        for &user in &users {
            around(next, user, &mut found, &mut is);
        }
        found.extend(users);
    }
    let faces: Vec<EntityRef> =
        found.iter().filter_map(|&l| typed::<Loop>(l)).filter_map(|l| get(next, l)).map(|l| l.face.entity()).collect();
    found.extend(faces);
    found.retain(|&entity| next.is_live(entity));
    found
}

/// Adds each item of `walk`.
fn add<K: Kind>(out: &mut BTreeSet<EntityRef>, walk: Walk<Id<K>>) {
    out.extend(walk.items().into_iter().map(Id::entity));
}

/// Whether `kind` is a curve or surface, which something may lie on.
const fn geometry(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Surface | EntityKind::Curve | EntityKind::ParamCurve)
}

/// Adds the uses of a loop or wire from `first`, and their edges.
fn add_uses(model: &Model, first: Id<EdgeUse>, out: &mut BTreeSet<EntityRef>) {
    for u in uses_from(model, first).items() {
        out.insert(u.entity());
        out.extend(get(model, u).map(|r| r.edge.entity()));
    }
}

/// Adds what `entity` links to in `model`, and the lists and rings it sits in there, each unless `added` holds it.
fn around(model: &Model, entity: EntityRef, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
    let mut links = Vec::new();
    note();
    model.links(entity, &mut links);
    out.extend(links);
    match entity.kind {
        EntityKind::EdgeUse => typed::<EdgeUse>(entity).into_iter().for_each(|id| use_around(model, id, out, added)),
        EntityKind::Edge => typed::<Edge>(entity).into_iter().for_each(|id| edge_around(model, id, out, added)),
        EntityKind::Vertex => {
            if let Some(start) = typed::<Vertex>(entity).and_then(|id| get(model, id)).and_then(|v| v.star) {
                added.star(model, start, out);
            }
        }
        EntityKind::Loop => typed::<Loop>(entity).into_iter().for_each(|id| loop_around(model, id, out, added)),
        EntityKind::Face => {
            out.extend(typed::<Face>(entity).into_iter().flat_map(|id| face_edges(model, id)).map(Id::entity));
            let outer = typed::<Face>(entity).and_then(|id| get(model, id)).map(|f| f.outer);
            added.list(out, entity, || list(model, outer, |l| l.next));
        }
        EntityKind::Shell => {
            if let Some(body) = typed::<Shell>(entity).and_then(|id| get(model, id)).map(|s| s.body) {
                shells_of(model, body, out, added);
            }
        }
        EntityKind::Wire => typed::<Wire>(entity).into_iter().for_each(|id| wire_around(model, id, out, added)),
        EntityKind::Body => {
            let Some(id) = typed::<Body>(entity) else { return };
            shells_of(model, id, out, added);
            let first = get(model, id).and_then(|b| b.first_wire);
            added.list(out, entity, || list(model, first, |w| w.next));
        }
        EntityKind::Surface
        | EntityKind::Curve
        | EntityKind::ParamCurve
        | EntityKind::Model
        | EntityKind::Section
        | EntityKind::Region => {}
    }
}

/// A body's list of shells.
fn shells_of(model: &Model, id: Id<Body>, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
    added.list(out, id.entity(), || list(model, get(model, id).and_then(|b| b.first_shell), |s| s.next));
}

/// A use's owner's list and its edge's ring.
fn use_around(model: &Model, id: Id<EdgeUse>, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
    let Some(record) = get(model, id) else { return };
    let owner = match record.owner {
        UseOwner::Loop(l) => l.entity(),
        UseOwner::Wire(w) => w.entity(),
    };
    let first = || first_use(model, record.owner).map_or(Walk::End(Vec::new()), |first| uses_from(model, first));
    added.list(out, owner, first);
    if let Some(edge) = get(model, record.edge) {
        added.list(out, record.edge.entity(), || radial(model, edge));
    }
}

/// An edge's ring of uses and their owners, and the stars at its ends.
fn edge_around(model: &Model, id: Id<Edge>, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
    let Some(record) = get(model, id) else { return };
    for u in radial(model, record).items() {
        out.insert(u.entity());
        out.extend(get(model, u).map(|r| match r.owner {
            UseOwner::Loop(l) => l.entity(),
            UseOwner::Wire(w) => w.entity(),
        }));
    }
    for end in [End::Start, End::Finish] {
        added.star(model, Around { edge: id, end }, out);
    }
}

/// A loop's face's list of loops, and its uses and their edges.
fn loop_around(model: &Model, id: Id<Loop>, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
    let Some(record) = get(model, id) else { return };
    added.list(out, record.face.entity(), || list(model, get(model, record.face).map(|f| f.outer), |l| l.next));
    add_uses(model, record.first, out);
}

/// A wire's body's list of wires, and its uses and their edges.
fn wire_around(model: &Model, id: Id<Wire>, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
    let Some(record) = get(model, id) else { return };
    added.list(out, record.body.entity(), || {
        list(model, get(model, record.body).and_then(|b| b.first_wire), |w| w.next)
    });
    add_uses(model, record.first, out);
}

/// The edges of every face of a shell, and its faces.
fn shell_edges(model: &Model, id: Id<Shell>, out: &mut BTreeSet<EntityRef>) {
    for face in faces(model, id) {
        out.insert(face.entity());
        out.extend(face_edges(model, face).into_iter().map(Id::entity));
    }
}

/// Adds what moving a shell, removing one, or changing a body's kind can affect; [`scope`] adds what lies on a curve or
/// surface taken away or replaced.
fn further(base: &Model, next: &Model, entity: EntityRef, out: &mut BTreeSet<EntityRef>, added: &mut Added) {
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
                    wire_around(next, wire, out, added);
                }
            }
        }
        _ => {}
    }
}

/// Everything in `model` that names one of the curves and surfaces `geometry`, found in one scan that reads, and
/// counts, every face, edge and use.
fn lying_on(model: &Model, geometry: &BTreeSet<EntityRef>) -> BTreeSet<EntityRef> {
    let mut out = BTreeSet::new();
    let faces =
        Face::table(model).records().inspect(|_| note()).filter(|(_, _, r)| geometry.contains(&r.surface.entity()));
    out.extend(faces.map(|(slot, generation, _)| Id::<Face>::new(slot, generation).entity()));
    let edges =
        Edge::table(model).records().inspect(|_| note()).filter(|(_, _, r)| geometry.contains(&r.curve.entity()));
    out.extend(edges.map(|(slot, generation, _)| Id::<Edge>::new(slot, generation).entity()));
    let uses = EdgeUse::table(model).records().inspect(|_| note());
    let uses = uses.filter(|(_, _, r)| r.pcurve.is_some_and(|p| geometry.contains(&p.entity())));
    out.extend(uses.map(|(slot, generation, _)| Id::<EdgeUse>::new(slot, generation).entity()));
    out
}
