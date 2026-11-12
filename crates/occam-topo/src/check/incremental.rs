//! Source: original
//! What [`super::body`] would say about the shells a commit changed, reached from what changed rather than by walking
//! them (P1), on the ground that the snapshot the edit began from passed every check.
//!
//! - Genus: the change in `V - E + 2F - L` of each closed shell, from the entities whose shells changed. A shell whose
//!   characteristic did not rise, by an even step, keeps a whole genus, and a new shell's characteristic is the change;
//!   any other shell, and every shell of a body whose kind changed, is counted whole.
//! - Joining, region by region: the faces whose neighbours through edges changed, and the faces that left the shell,
//!   fall into regions, each joined through faces adjacent before or after the edit. A region's border is the faces of
//!   the shell next to it that the edit did not touch, whose neighbours are as they were. The shell is still joined if,
//!   in each region, its faces in the shell and its border are joined to each other through them alone: before the edit
//!   the shell was joined, and a path between two of its untouched faces leaves and re-enters each region it crosses at
//!   two faces of that region's border, which the region joins again. A region with no border is the whole of what
//!   was the shell, or new to it: it is enough only in a new shell, or where it holds a face that was the shell's own,
//!   and as the only region. Where a region falls short, as a band cut from a ring or a shell cut in two does, the
//!   shell is searched whole, as before: its touched faces must be joined to each other, and a shell that was there
//!   before must find among them a face that was already its own, or else the face it begins from joins them. So an
//!   edit's cost follows what it touched, however scattered.

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

/// The faces of one shell an edit touched: those in it in the model it would commit, and those that left it.
#[derive(Default)]
struct Sides {
    /// Faces in the shell after the edit whose neighbours through edges may have changed.
    live: BTreeSet<Id<Face>>,
    /// Faces in the shell before the edit that are gone from it.
    gone: BTreeSet<Id<Face>>,
}

#[cfg(test)]
thread_local! {
    static WHOLE: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Counts one search of a whole shell, in tests.
fn searched_whole() {
    #[cfg(test)]
    WHOLE.with(|whole| whole.set(whole.get().saturating_add(1)));
}

/// How many times a commit has searched a whole shell on this thread.
#[cfg(test)]
pub(crate) fn whole_searches() -> usize {
    WHOLE.with(core::cell::Cell::get)
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
    let mut shells: BTreeMap<Id<Shell>, Sides> = BTreeMap::new();
    for face in faces {
        let (was, is) = (get(touched.base, face).map(|r| r.shell), get(touched.next, face).map(|r| r.shell));
        if let Some(shell) = is {
            shells.entry(shell).or_default().live.insert(face);
        }
        if let Some(shell) = was.filter(|&shell| is != Some(shell)) {
            shells.entry(shell).or_default().gone.insert(face);
        }
    }
    for (shell, sides) in shells {
        if sides.live.is_empty() || regions_join(touched, shell, &sides) {
            continue;
        }
        searched_whole();
        let mut targets = sides.live;
        let Some(start) = start(touched, shell, &mut targets) else { continue };
        let found = reach(touched.next, start, |f| get(touched.next, f).map(|r| r.shell) == Some(shell), &targets);
        if let Some(apart) = targets.difference(&found).next() {
            out.push(failed(Reason::ShellNotConnected, &[shell.entity(), apart.entity()]));
        }
    }
}

/// The faces next to `face` through its edges in `model`, itself left out.
fn adjacent(model: &Model, face: Id<Face>) -> impl Iterator<Item = Id<Face>> {
    face_edges(model, face).into_iter().flat_map(move |e| edge_faces(model, e)).filter(move |&f| f != face)
}

/// The regions of what an edit touched of `shell`: its touched faces joined through faces adjacent in either model,
/// each region in the order of its least face.
fn regions(touched: &Touched<'_>, shell: Id<Shell>, sides: &Sides) -> Vec<BTreeSet<Id<Face>>> {
    let nodes: BTreeSet<Id<Face>> = sides.live.union(&sides.gone).copied().collect();
    let mut left = nodes.clone();
    let mut found = Vec::new();
    while let Some(first) = left.pop_first() {
        let (mut region, mut frontier) = (BTreeSet::from([first]), vec![first]);
        while let Some(face) = frontier.pop() {
            for model in touched.models() {
                if get(model, face).map(|r| r.shell) != Some(shell) {
                    continue;
                }
                for next in adjacent(model, face).filter(|f| nodes.contains(f)).collect::<Vec<_>>() {
                    if left.remove(&next) {
                        region.insert(next);
                        frontier.push(next);
                    }
                }
            }
        }
        found.push(region);
    }
    found
}

/// Whether every region of what the edit touched of `shell` joins its faces in the shell and its border through them
/// alone, which joins the shell (the module's account); `false` where one falls short and the shell must be searched
/// whole.
fn regions_join(touched: &Touched<'_>, shell: Id<Shell>, sides: &Sides) -> bool {
    let next = touched.next;
    let in_next = |f: Id<Face>| get(next, f).map(|r| r.shell) == Some(shell);
    let in_base = |f: Id<Face>| get(touched.base, f).map(|r| r.shell) == Some(shell);
    let all = regions(touched, shell, sides);
    let held = all.iter().filter(|region| region.iter().any(|f| sides.live.contains(f))).count();
    let existed = get(touched.base, shell).is_some();
    all.iter().all(|region| {
        let live: BTreeSet<Id<Face>> = region.intersection(&sides.live).copied().collect();
        let Some(&first) = live.first() else { return true };
        let border: BTreeSet<Id<Face>> = live
            .iter()
            .flat_map(|&f| adjacent(next, f).collect::<Vec<_>>())
            .filter(|f| !region.contains(f) && !sides.gone.contains(f) && in_next(*f))
            .collect();
        if border.is_empty() && (held > 1 || (existed && !region.iter().any(|&f| in_base(f)))) {
            return false;
        }
        let allowed: BTreeSet<Id<Face>> = live.union(&border).copied().collect();
        reach(next, first, |f| allowed.contains(&f), &allowed).len() == allowed.len()
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::tests::clean;
    use crate::models::cube;

    #[test]
    fn each_kind_reaches_the_faces_and_edges_it_bounds_and_the_entities_whose_counts_it_may_change() {
        // On the cube: a use bounds its loop's face and its edge's ring, a loop its face and its edges' rings, an edge
        // its own ring, and a shell the face it begins from; a vertex's count may change with it, an edge's and its
        // ends' with the edge, and with each of its uses.
        let (snapshot, built) = clean(&cube());
        let m = &snapshot.model;
        let u = built.uses[0];
        let (e, UseOwner::Loop(l)) = get(m, u).map(|r| (r.edge, r.owner)).unwrap() else { panic!("a loop's use") };
        let face = get(m, l).unwrap().face;
        let ends = get(m, e).unwrap().ends;
        let shell = built.shell.unwrap();
        let reach = |entity: EntityRef| {
            let (mut faces, mut edges) = (BTreeSet::new(), BTreeSet::new());
            neighbours(m, entity, &mut faces, &mut edges);
            (faces, edges)
        };
        assert_eq!(reach(u.entity()), (BTreeSet::from([face]), BTreeSet::from([e])));
        assert_eq!(reach(l.entity()), (BTreeSet::from([face]), face_edges(m, face).into_iter().collect()));
        assert_eq!(reach(e.entity()), (BTreeSet::new(), BTreeSet::from([e])));
        assert_eq!(reach(shell.entity()), (BTreeSet::from([get(m, shell).unwrap().face]), BTreeSet::new()));
        let count = |entity: EntityRef| {
            let mut out = BTreeSet::new();
            counted(m, entity, &mut out);
            out
        };
        let edge_and_ends = BTreeSet::from([e.entity(), ends.0.entity(), ends.1.entity()]);
        assert_eq!(count(ends.0.entity()), BTreeSet::from([ends.0.entity()]));
        assert_eq!(count(e.entity()), edge_and_ends);
        assert_eq!(count(u.entity()), edge_and_ends);
    }
}
