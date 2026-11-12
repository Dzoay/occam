//! Source: original
//! What [`super::body`] would say about the shells a commit changed, reached from what changed rather than by walking
//! them (P1), on the ground that the snapshot the edit began from passed every check.
//!
//! - Genus: the change in `V - E + 2F - L` of each closed shell, from the entities whose shells changed. A shell whose
//!   characteristic did not rise, by an even step, keeps a whole genus, and a new shell's characteristic is the change;
//!   any other shell, and every shell of a body whose kind changed, is counted whole.
//! - Joining: the faces whose neighbours through edges changed must still be joined to each other within their shell,
//!   which, with every other face joined to one of them as before, joins the shell. A shell that was there before must
//!   find among them a face that was already its own; where none is, as for a face given to it without joining it, the
//!   face it begins from joins them. The search stops once it has met them all.

use std::collections::{BTreeMap, BTreeSet};

use occam_report::{EntityKind, EntityRef, Reason, Report};

use super::failed;
use super::read::{body_kind, edge_shells, get, list, loop_shell, typed, uses_from, vertex_edges};
use super::shell::{edge_faces, face_edges, genus as whole_shell_genus, loops, reach, whole_genus};
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
}

impl<'a> Touched<'a> {
    /// What `edit` touched.
    pub(crate) fn of(edit: &'a Edit) -> Self {
        let entities = edit.created.iter().chain(&edit.changed).chain(&edit.deleted).copied().collect();
        Self { base: &edit.base.model, next: &edit.next, entities }
    }

    /// Both models, the one the edit began from first.
    const fn models(&self) -> [&'a Model; 2] {
        [self.base, self.next]
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
        EntityKind::Shell => faces.extend(typed::<Shell>(entity).and_then(|s| get(model, s)).map(|s| s.face)),
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
    for (shell, mut targets) in shells {
        let Some(start) = start(touched, shell, &mut targets) else { continue };
        let found = reach(touched.next, start, |f| get(touched.next, f).map(|r| r.shell) == Some(shell), &targets);
        if let Some(apart) = targets.difference(&found).next() {
            out.push(failed(Reason::ShellNotConnected, &[shell.entity(), apart.entity()]));
        }
    }
}

/// Where the search of `shell` begins: its first target, unless the shell was there before and no target was already
/// its own, when the face it begins from joins the targets and the search begins there. `None` if that face is not its
/// own, which the shell's own check reports.
fn start(touched: &Touched<'_>, shell: Id<Shell>, targets: &mut BTreeSet<Id<Face>>) -> Option<Id<Face>> {
    let own = |model: &Model, f: Id<Face>| get(model, f).map(|r| r.shell) == Some(shell);
    if get(touched.base, shell).is_none() || targets.iter().any(|&f| own(touched.base, f)) {
        return targets.first().copied();
    }
    let anchor = get(touched.next, shell).map(|s| s.face).filter(|&f| own(touched.next, f))?;
    targets.insert(anchor);
    Some(anchor)
}
