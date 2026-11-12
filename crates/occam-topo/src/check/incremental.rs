//! Source: original
//! What [`super::body`] would say about the shells a commit changed, reached from what changed rather than by walking
//! them (P1), on the ground that the snapshot the edit began from passed every check.
//!
//! - Lists of faces: each face is listed once, by a face of its own shell or by its shell, counted from the links the
//!   edit changed. A list the edit closes on itself is found where every link round it is one the edit wrote; one that
//!   closes through faces the edit left alone is found by [`super::model`], since a list linked one way cannot show
//!   from a few links that it still reaches its head.
//! - Genus: the change in `V - E + 2F - L` of each closed shell, from the entities whose shells changed. A shell whose
//!   characteristic did not rise, by an even step, keeps a whole genus, and a new shell's characteristic is the change;
//!   any other shell, and every shell of a body whose kind changed, is counted whole.
//! - Joining: the faces whose neighbours through edges changed must still be joined to each other within their shell,
//!   which, with every other face joined to one of them as before, joins the shell. The search stops once it has met
//!   them all.

use std::collections::{BTreeMap, BTreeSet};

use occam_report::{EntityKind, EntityRef, Reason, Report};

use super::failed;
use super::read::{body_kind, edge_shells, get, list, loop_shell, typed, uses_from, vertex_edges};
use super::shell::{edge_faces, face_edges, faces, genus as whole_shell_genus, loops, reach, whole_genus};
use crate::edit::Edit;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Loop, Shell, Vertex};
use crate::records::{BodyKind, UseOwner};
use crate::snapshot::Model;

/// What an edit touched, and the models before and after it.
pub(crate) struct Touched<'a> {
    /// The model the edit began from.
    base: &'a Model,
    /// The model it would commit.
    next: &'a Model,
    /// What it created, changed or removed.
    entities: Vec<EntityRef>,
    /// What it created or changed.
    edited: BTreeSet<EntityRef>,
}

impl<'a> Touched<'a> {
    /// What `edit` touched.
    pub(crate) fn of(edit: &'a Edit) -> Self {
        let edited: BTreeSet<EntityRef> = edit.created.union(&edit.changed).copied().collect();
        let entities = edited.iter().chain(&edit.deleted).copied().collect();
        Self { base: &edit.base.model, next: &edit.next, entities, edited }
    }

    /// Both models, the one the edit began from first.
    const fn models(&self) -> [&'a Model; 2] {
        [self.base, self.next]
    }
}

/// The face a shell's or a face's list link names, in `model`.
fn list_link(model: &Model, entity: EntityRef) -> Option<Id<Face>> {
    match entity.kind {
        EntityKind::Shell => typed::<Shell>(entity).and_then(|s| get(model, s)).map(|s| s.first_face),
        EntityKind::Face => typed::<Face>(entity).and_then(|f| get(model, f)).and_then(|f| f.next),
        _ => None,
    }
}

/// How the links that list a face changed: their net count, and what lists it now among what the edit wrote.
#[derive(Default)]
struct Listing {
    net: i64,
    by: Vec<EntityRef>,
}

/// Reports each face that the edit leaves listed other than once, by a list of its own shell.
pub(crate) fn face_lists(touched: &Touched<'_>, out: &mut Vec<Report>) {
    let mut listings: BTreeMap<Id<Face>, Listing> = BTreeMap::new();
    let mut faces: BTreeSet<Id<Face>> = touched.entities.iter().filter_map(|&e| typed(e)).collect();
    for &entity in &touched.entities {
        if let Some(face) = list_link(touched.base, entity) {
            let listing = listings.entry(face).or_default();
            listing.net = listing.net.saturating_sub(1);
        }
        if let Some(face) = list_link(touched.next, entity) {
            let listing = listings.entry(face).or_default();
            listing.net = listing.net.saturating_add(1);
            listing.by.push(entity);
        }
    }
    faces.extend(listings.keys());
    let net = |face: Id<Face>| {
        let before = i64::from(touched.base.is_live(face.entity()));
        before.saturating_add(listings.get(&face).map_or(0, |l| l.net))
    };
    for face in faces {
        listed_once(touched, face, net(face), listings.get(&face).and_then(|l| l.by.first().copied()), out);
    }
    closed(touched, net, out);
}

/// Reports `face` if `net` links list it other than once, by its own shell's list.
fn listed_once(touched: &Touched<'_>, face: Id<Face>, net: i64, by: Option<EntityRef>, out: &mut Vec<Report>) {
    let Some(record) = get(touched.next, face) else {
        let lister = (net > 0 && by.is_none()).then(|| lister(touched.base, face)).flatten();
        out.extend(lister.map(|l| failed(Reason::DeadLink, &[l.entity(), face.entity()])));
        return;
    };
    let shell = record.shell;
    if net < 1 {
        out.push(failed(Reason::NotListed, &[face.entity(), shell.entity()]));
    } else if net > 1 {
        out.push(failed(Reason::ListBroken, &[shell.entity(), face.entity()]));
    } else if let Some(by) = by {
        let by_shell = typed::<Face>(by).and_then(|f| get(touched.next, f)).map(|f| f.shell);
        if let Some(other) = by_shell.filter(|&s| s != shell) {
            out.push(failed(Reason::ListedElsewhere, &[other.entity(), face.entity()]));
        }
    } else if let Some(old) = get(touched.base, face).map(|f| f.shell).filter(|&s| s != shell)
        && lister(touched.base, face).is_some()
    {
        out.push(failed(Reason::ListedElsewhere, &[old.entity(), face.entity()]));
    }
}

/// The face before `face` in its shell's list in `base`, if a face rather than the shell lists it. A shell that lists a
/// face checks that itself.
fn lister(base: &Model, face: Id<Face>) -> Option<Id<Face>> {
    let shell = get(base, face)?.shell;
    faces(base, shell).items().into_iter().find(|&f| get(base, f).and_then(|r| r.next) == Some(face))
}

/// Reports each list of faces the edit closes on itself through links it wrote, where no face is listed twice.
fn closed(touched: &Touched<'_>, count: impl Fn(Id<Face>) -> i64, out: &mut Vec<Report>) {
    let mut done = BTreeSet::new();
    for start in touched.edited.iter().filter_map(|&e| typed::<Face>(e)) {
        let mut path = BTreeSet::new();
        let mut at = start;
        while !done.contains(&at) {
            if !path.insert(at) {
                let shell = get(touched.next, at).map(|f| f.shell.entity());
                if count(at) == 1 {
                    out.push(failed(Reason::ListBroken, &shell.into_iter().chain([at.entity()]).collect::<Vec<_>>()));
                }
                break;
            }
            let following = get(touched.next, at).and_then(|f| f.next);
            match following.filter(|_| touched.edited.contains(&at.entity())) {
                Some(next) => at = next,
                None => break,
            }
        }
        done.extend(path);
    }
}

/// What an entity adds to a closed shell's `V - E + 2F - L`.
const fn weight(kind: EntityKind) -> i64 {
    match kind {
        EntityKind::Vertex => 1,
        EntityKind::Face => 2,
        EntityKind::Edge | EntityKind::Loop => -1,
        _ => 0,
    }
}

/// The shells `entity` counts in, in `model`: a vertex once in each shell its edges lie in.
fn shells_of(model: &Model, entity: EntityRef) -> BTreeSet<Id<Shell>> {
    match entity.kind {
        EntityKind::Face => typed::<Face>(entity).and_then(|f| get(model, f)).map(|f| f.shell).into_iter().collect(),
        EntityKind::Loop => typed::<Loop>(entity).and_then(|l| loop_shell(model, l)).into_iter().collect(),
        EntityKind::Edge => typed::<Edge>(entity).map(|e| edge_shells(model, e)).unwrap_or_default(),
        EntityKind::Vertex => {
            let edges = typed::<Vertex>(entity).map(|v| vertex_edges(model, v)).unwrap_or_default();
            edges.into_iter().flat_map(|e| edge_shells(model, e)).collect()
        }
        _ => BTreeSet::new(),
    }
}

/// Adds the vertices, edges, faces and loops whose shells may change with `entity`, in `model`.
fn counted(model: &Model, entity: EntityRef, out: &mut BTreeSet<EntityRef>) {
    let edge = |id: Id<Edge>, out: &mut BTreeSet<EntityRef>| {
        out.insert(id.entity());
        out.extend(get(model, id).into_iter().flat_map(|r| [r.ends.0.entity(), r.ends.1.entity()]));
    };
    let a_loop = |id: Id<Loop>, out: &mut BTreeSet<EntityRef>| {
        out.insert(id.entity());
        let uses = get(model, id).into_iter().flat_map(|r| uses_from(model, r.first).items());
        for e in uses.filter_map(|u| get(model, u).map(|r| r.edge)).collect::<Vec<_>>() {
            edge(e, out);
        }
    };
    match entity.kind {
        EntityKind::Vertex => {
            out.insert(entity);
        }
        EntityKind::Edge => typed(entity).into_iter().for_each(|e| edge(e, out)),
        EntityKind::EdgeUse => {
            let e = typed::<EdgeUse>(entity).and_then(|u| get(model, u)).map(|u| u.edge);
            if let Some(e) = e {
                edge(e, out);
            }
        }
        EntityKind::Loop => typed(entity).into_iter().for_each(|l| a_loop(l, out)),
        EntityKind::Face => {
            out.insert(entity);
            let all = typed::<Face>(entity).map(|f| loops(model, f).items()).unwrap_or_default();
            for l in all {
                a_loop(l, out);
            }
        }
        _ => {}
    }
}

/// Reports each closed shell the edit changed whose counts give no whole genus.
pub(crate) fn genus(touched: &Touched<'_>, out: &mut Vec<Report>) {
    let mut entities = BTreeSet::new();
    for &entity in &touched.entities {
        touched.models().into_iter().for_each(|m| counted(m, entity, &mut entities));
    }
    let mut change: BTreeMap<Id<Shell>, i64> = BTreeMap::new();
    for entity in entities {
        let w = weight(entity.kind);
        for (model, sign) in [(touched.base, -1), (touched.next, 1)] {
            for shell in shells_of(model, entity) {
                let c = change.entry(shell).or_default();
                *c = c.saturating_add(w.saturating_mul(sign));
            }
        }
    }
    let mut shells: BTreeSet<Id<Shell>> = change.keys().copied().collect();
    shells.extend(touched.entities.iter().filter_map(|&e| typed::<Shell>(e)));
    for body in touched.entities.iter().filter_map(|&e| typed::<Body>(e)) {
        let (was, is) = (get(touched.base, body).map(|b| b.kind), get(touched.next, body));
        if let Some(record) = is.filter(|b| was != Some(b.kind)) {
            shells.extend(list(touched.next, record.first_shell, |s| s.next).items());
        }
    }
    for shell in shells {
        let Some(record) = get(touched.next, shell) else { continue };
        if body_kind(touched.next, record.body) != Some(BodyKind::Solid) {
            continue;
        }
        let d = change.get(&shell).copied().unwrap_or(0);
        let decided = get(touched.base, shell).map_or(whole_genus(d), |old| {
            old.body == record.body
                && body_kind(touched.base, old.body) == Some(BodyKind::Solid)
                && d & 1 == 0
                && d <= 0
        });
        if !decided {
            whole_shell_genus(touched.next, shell, out);
        }
    }
}

/// Adds the faces `entity` bounds or lies on, and the edges whose rings of uses it may change, in `model`.
fn neighbours(model: &Model, entity: EntityRef, faces: &mut BTreeSet<Id<Face>>, edges: &mut BTreeSet<Id<Edge>>) {
    match entity.kind {
        EntityKind::Face => {
            let Some(face) = typed::<Face>(entity) else { return };
            faces.insert(face);
            edges.extend(face_edges(model, face));
        }
        EntityKind::Loop => {
            let Some(record) = typed::<Loop>(entity).and_then(|l| get(model, l)) else { return };
            faces.insert(record.face);
            let uses = uses_from(model, record.first).items();
            edges.extend(uses.into_iter().filter_map(|u| get(model, u).map(|r| r.edge)));
        }
        EntityKind::EdgeUse => {
            let Some(record) = typed::<EdgeUse>(entity).and_then(|u| get(model, u)) else { return };
            edges.insert(record.edge);
            if let UseOwner::Loop(l) = record.owner {
                faces.extend(get(model, l).map(|r| r.face));
            }
        }
        EntityKind::Edge => edges.extend(typed::<Edge>(entity)),
        EntityKind::Shell => faces.extend(typed::<Shell>(entity).and_then(|s| get(model, s)).map(|s| s.first_face)),
        _ => {}
    }
}

/// Reports each shell the edit changed whose faces are no longer all joined through their edges.
pub(crate) fn joined(touched: &Touched<'_>, out: &mut Vec<Report>) {
    let (mut faces, mut edges) = (BTreeSet::new(), BTreeSet::new());
    for &entity in &touched.entities {
        touched.models().into_iter().for_each(|m| neighbours(m, entity, &mut faces, &mut edges));
    }
    for edge in edges {
        touched.models().into_iter().for_each(|m| faces.extend(edge_faces(m, edge)));
    }
    let mut shells: BTreeMap<Id<Shell>, BTreeSet<Id<Face>>> = BTreeMap::new();
    for face in faces {
        if let Some(record) = get(touched.next, face) {
            shells.entry(record.shell).or_default().insert(face);
        }
    }
    for (shell, targets) in shells {
        let Some(&start) = targets.first() else { continue };
        let found = reach(touched.next, start, |f| get(touched.next, f).map(|r| r.shell) == Some(shell), &targets);
        if let Some(apart) = targets.difference(&found).next() {
            out.push(failed(Reason::ShellNotConnected, &[shell.entity(), apart.entity()]));
        }
    }
}
