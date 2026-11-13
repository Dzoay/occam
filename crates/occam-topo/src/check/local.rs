//! Source: original
//! The invariants of one entity, read from its record and its neighbours' (PARITY 1.10): its links are live; loops come
//! round and wires end or come round, each use beginning where the one before ends; owners and what they list name each
//! other; the uses round an edge and the edges round a vertex form rings; and an edge has the uses its body's kind
//! allows, in opposite directions. Each failure is reported once, by the entity whose list or ring it is: a member checks
//! only that it is listed, and only when the list is sound.

use std::collections::BTreeSet;

use occam_report::{EntityKind, EntityRef, NamedValue, Quantity, Reason, Report};

use super::read::{
    Walk, body_kind, end_vertex, first_use, get, list, loop_face, note, radial, star, typed, use_body, use_ends,
    use_shell, uses_from,
};
use super::{culprit, failed, holds};
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Loop, Shell, Vertex, Wire};
use crate::records::{Around, BodyKind, End, UseOwner};
use crate::snapshot::Model;

/// Every invariant of `entity`. If any of its links is not live, that alone is reported, as the rest would follow from
/// it.
pub(crate) fn check(model: &Model, entity: EntityRef, out: &mut Vec<Report>) {
    if dead_links(model, entity, out) {
        return;
    }
    match entity.kind {
        EntityKind::Body => typed(entity).into_iter().for_each(|id| body(model, id, out)),
        EntityKind::Shell => typed(entity).into_iter().for_each(|id| shell(model, id, out)),
        EntityKind::Face => typed(entity).into_iter().for_each(|id| face(model, id, out)),
        EntityKind::Loop => typed(entity).into_iter().for_each(|id| a_loop(model, id, out)),
        EntityKind::Wire => typed(entity).into_iter().for_each(|id| wire(model, id, out)),
        EntityKind::EdgeUse => typed(entity).into_iter().for_each(|id| a_use(model, id, out)),
        EntityKind::Edge => typed(entity).into_iter().for_each(|id| edge(model, id, out)),
        EntityKind::Vertex => typed(entity).into_iter().for_each(|id| vertex(model, id, out)),
        EntityKind::Surface
        | EntityKind::Curve
        | EntityKind::ParamCurve
        | EntityKind::Model
        | EntityKind::Section
        | EntityKind::Region => {}
    }
}

/// Reports each link of `entity` to an entity that is not live, each once, in id order; says whether there was one.
fn dead_links(model: &Model, entity: EntityRef, out: &mut Vec<Report>) -> bool {
    let mut links = Vec::new();
    note();
    model.links(entity, &mut links);
    let dead: BTreeSet<EntityRef> = links.into_iter().filter(|&link| !model.is_live(link)).collect();
    out.extend(dead.iter().map(|&link| failed(Reason::DeadLink, &[entity, link])));
    !dead.is_empty()
}

/// Reports `id` if `listed`, a sound list of its owner's, does not hold it.
fn listed<K: crate::kind::Kind>(out: &mut Vec<Report>, id: Id<K>, owner: EntityRef, listed: Option<&[Id<K>]>) {
    if listed.is_some_and(|items| !items.contains(&id)) {
        out.push(failed(Reason::NotListed, &[id.entity(), owner]));
    }
}

/// A body: its shells and wires name it, and it holds what its kind allows.
fn body(model: &Model, id: Id<Body>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let shells = list(model, record.first_shell, |s| s.next);
    holds(out, id.entity(), &shells, false, Reason::ListBroken, |s| get(model, s).map(|r| r.body) == Some(id));
    let wires = list(model, record.first_wire, |w| w.next);
    holds(out, id.entity(), &wires, false, Reason::ListBroken, |w| get(model, w).map(|r| r.body) == Some(id));
    let fits = match record.kind {
        BodyKind::Solid | BodyKind::Sheet => record.first_shell.is_some() && record.first_wire.is_none(),
        BodyKind::Wire => record.first_wire.is_some() && record.first_shell.is_none(),
    };
    if !fits {
        out.push(failed(Reason::BodyKindMismatch, &[id.entity()]));
    }
}

/// A shell: its body lists it, and the face it begins from names it. Which faces it holds, those that name it and are
/// joined to that one, is a whole-shell matter.
fn shell(model: &Model, id: Id<Shell>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let shells = list(model, get(model, record.body).and_then(|b| b.first_shell), |s| s.next);
    listed(out, id, record.body.entity(), shells.sound(false));
    if get(model, record.face).map(|f| f.shell) != Some(id) {
        out.push(failed(Reason::ListedElsewhere, &[id.entity(), record.face.entity()]));
    }
}

/// A face: its loops, from the outer, end and name it.
fn face(model: &Model, id: Id<Face>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let loops = list(model, Some(record.outer), |l| l.next);
    holds(out, id.entity(), &loops, false, Reason::ListBroken, |l| loop_face(model, l) == Some(id));
}

/// A loop: its face lists it, and its uses come round, name it, and meet.
fn a_loop(model: &Model, id: Id<Loop>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let loops = list(model, get(model, record.face).map(|f| f.outer), |l| l.next);
    listed(out, id, record.face.entity(), loops.sound(false));
    let uses = uses_from(model, record.first);
    let owner = UseOwner::Loop(id);
    let named = |u: Id<EdgeUse>| get(model, u).map(|r| r.owner) == Some(owner);
    if let Some(items) = holds(out, id.entity(), &uses, true, Reason::LoopNotClosed, named) {
        meet(model, &items, true, out);
    }
}

/// A wire: its body lists it, and its uses end or come round, name it, and meet.
fn wire(model: &Model, id: Id<Wire>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let wires = list(model, get(model, record.body).and_then(|b| b.first_wire), |w| w.next);
    listed(out, id, record.body.entity(), wires.sound(false));
    let uses = uses_from(model, record.first);
    let ring = matches!(uses, Walk::Ring(_));
    let owner = UseOwner::Wire(id);
    let named = |u: Id<EdgeUse>| get(model, u).map(|r| r.owner) == Some(owner);
    if let Some(items) = holds(out, id.entity(), &uses, ring, Reason::ListBroken, named) {
        meet(model, &items, ring, out);
    }
}

/// Reports each use in `uses` that does not begin where the one before it ends, round the ring if `ring`.
fn meet(model: &Model, uses: &[Id<EdgeUse>], ring: bool, out: &mut Vec<Report>) {
    let wrap = uses.first().filter(|_| ring);
    let pairs = uses.iter().zip(uses.iter().skip(1).chain(wrap));
    for (&before, &after) in pairs {
        let (Some((_, end)), Some((begin, _))) = (use_ends(model, before), use_ends(model, after)) else { continue };
        if end != begin {
            out.push(failed(Reason::UsesDoNotMeet, &[before.entity(), after.entity()]));
        }
    }
}

/// The entity a use's owner is.
const fn owner_entity(owner: UseOwner) -> EntityRef {
    match owner {
        UseOwner::Loop(l) => l.entity(),
        UseOwner::Wire(w) => w.entity(),
    }
}

/// A use: its owner lists it, and it is in the ring of its edge's uses.
fn a_use(model: &Model, id: Id<EdgeUse>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    if let Some(first) = first_use(model, record.owner) {
        let uses = uses_from(model, first);
        let sound = match record.owner {
            UseOwner::Loop(_) => uses.sound(true),
            UseOwner::Wire(_) => uses.sound(false).or_else(|| uses.sound(true)),
        };
        listed(out, id, owner_entity(record.owner), sound);
    }
    let Some(edge) = get(model, record.edge) else { return };
    if edge.first_use.is_none() {
        out.push(failed(Reason::NotListed, &[id.entity(), record.edge.entity()]));
    } else {
        listed(out, id, record.edge.entity(), radial(model, edge).sound(true));
    }
}

/// An edge: each end is in its vertex's star, its uses form a ring that names it, and they are as many, in such
/// directions, as its body's kind allows.
fn edge(model: &Model, id: Id<Edge>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let mut unlisted = BTreeSet::new();
    for end in [End::Start, End::Finish] {
        let vertex = end_vertex(record, end);
        let place = Around { edge: id, end };
        let start = get(model, vertex).and_then(|v| v.star);
        let missing = start.is_none_or(|start| star(model, start).sound(true).is_some_and(|ps| !ps.contains(&place)));
        if missing {
            unlisted.insert(vertex);
        }
    }
    out.extend(unlisted.into_iter().map(|v| failed(Reason::NotListed, &[id.entity(), v.entity()])));
    if record.first_use.is_none() {
        return;
    }
    let ring = radial(model, record);
    let named = |u: Id<EdgeUse>| get(model, u).map(|r| r.edge) == Some(id);
    if let Some(uses) = holds(out, id.entity(), &ring, true, Reason::ListBroken, named) {
        use_counts(model, id, &uses, out);
    }
}

/// The uses of edge `id`: in one shell, or one wire body, as many as its body's kind allows, two of them opposed.
fn use_counts(model: &Model, id: Id<Edge>, uses: &[Id<EdgeUse>], out: &mut Vec<Report>) {
    let shells: BTreeSet<Id<Shell>> = uses.iter().filter_map(|&u| use_shell(model, u)).collect();
    let bodies: BTreeSet<Id<Body>> = uses.iter().filter_map(|&u| use_body(model, u)).collect();
    let in_loops = uses.iter().filter(|&&u| get(model, u).is_some_and(|r| matches!(r.owner, UseOwner::Loop(_))));
    let mixed = (1..uses.len()).contains(&in_loops.count());
    if mixed || shells.len() > 1 || bodies.len() > 1 {
        // Two shells say where the edge is shared; otherwise the bodies do.
        let bodies = bodies.iter().map(|b| b.entity()).filter(|_| shells.len() < 2);
        let named = [id.entity()].into_iter().chain(shells.iter().map(|s| s.entity())).chain(bodies);
        out.push(failed(Reason::EdgeShared, &named.collect::<Vec<_>>()));
        return;
    }
    let (Some(&owner), n) = (bodies.first(), uses.len()) else { return };
    let allowed = match body_kind(model, owner) {
        Some(BodyKind::Solid) => n == 2,
        Some(BodyKind::Sheet) => n == 1 || n == 2,
        Some(BodyKind::Wire) => n == 1,
        None => true,
    };
    if !allowed {
        let count = NamedValue::new("uses", Quantity::Count, f64::from(u32::try_from(n).unwrap_or(u32::MAX)));
        out.push(failed(Reason::WrongUseCount, &[id.entity(), owner.entity()]).with_value(count));
        return;
    }
    let reversed = |u: Id<EdgeUse>| get(model, u).map(|r| r.reversed);
    if let [a, b] = uses
        && reversed(*a) == reversed(*b)
    {
        out.push(failed(Reason::UsesNotOpposed, &[id.entity(), a.entity(), b.entity()]));
    }
}

/// A vertex: its star comes round, and every edge in it ends at the vertex at the end named.
fn vertex(model: &Model, id: Id<Vertex>, out: &mut Vec<Report>) {
    let Some(start) = get(model, id).and_then(|v| v.star) else { return };
    let places = star(model, start);
    let Some(items) = places.sound(true) else {
        let at = culprit(&places).map(|p| p.edge.entity());
        out.push(failed(Reason::ListBroken, &[id.entity()].into_iter().chain(at).collect::<Vec<_>>()));
        return;
    };
    let mut elsewhere = BTreeSet::new();
    for place in items {
        if get(model, place.edge).map(|e| end_vertex(e, place.end)) != Some(id) {
            elsewhere.insert(place.edge);
        }
    }
    out.extend(elsewhere.into_iter().map(|e| failed(Reason::ListedElsewhere, &[id.entity(), e.entity()])));
}
