//! Source: mantyla-1988
//! The invariants of a whole body, shell by shell. A shell's faces are those that name it, and every one of them is
//! joined through their edges to the face the shell begins from, so that two pieces meeting only at a vertex are two
//! shells, the one check that sees a missing pinch (S1 item 6); and a closed shell's counts satisfy the Euler–Poincaré
//! formula `V - E + 2F - L = 2(1 - G)` for a whole genus `G` (Mäntylä 1988), where a vertex where shells meet counts once
//! in each. These read the whole shell; a commit reaches the same answers from what it changed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use occam_report::{NamedValue, Quantity, Reason, Report};

use super::failed;
use super::read::{Walk, get, list, loop_face, radial, uses_from};
use crate::kind::sealed::Sealed;
use crate::kind::{Body, Edge, Face, Id, Loop, Shell, Vertex};
use crate::records::{BodyKind, UseOwner};
use crate::snapshot::Model;

/// The faces of shell `id`: those that name it and are joined through their edges to the face it begins from, each
/// once, in the order a breadth-first walk from that face meets them. None if that face does not name it, which the
/// shell's own check reports.
pub(crate) fn faces(model: &Model, id: Id<Shell>) -> Vec<Id<Face>> {
    let named = |f: Id<Face>| get(model, f).map(|r| r.shell) == Some(id);
    let start = get(model, id).map(|s| s.face).filter(|&f| named(f));
    let mut found: Vec<Id<Face>> = start.into_iter().collect();
    let mut seen: BTreeSet<Id<Face>> = found.iter().copied().collect();
    // No shell has more faces than the table has slots, so a walk that has listed more stops, whatever it met.
    let (mut at, most) = (0_usize, Face::table(model).len());
    while let Some(&face) = found.get(at).filter(|_| found.len() <= most) {
        at = at.saturating_add(1);
        let next = face_edges(model, face).into_iter().flat_map(|e| edge_faces(model, e));
        let new: Vec<Id<Face>> = next.filter(|&f| named(f) && seen.insert(f)).collect();
        found.extend(new);
    }
    found
}

/// The faces that name each shell, in slot order: what the whole-model check holds each shell's walk to. Only a scan
/// of every face finds one that names a shell it is not joined to.
pub(crate) struct Members(BTreeMap<Id<Shell>, Vec<Id<Face>>>);

impl Members {
    /// Every live face of `model`, by the shell it names.
    pub(crate) fn of(model: &Model) -> Self {
        let mut by: BTreeMap<Id<Shell>, Vec<Id<Face>>> = BTreeMap::new();
        for (slot, generation, record) in Face::table(model).records() {
            by.entry(record.shell).or_default().push(Id::new(slot, generation));
        }
        Self(by)
    }
}

/// The loops of a face, from its outer.
pub(crate) fn loops(model: &Model, id: Id<Face>) -> Walk<Id<Loop>> {
    list(model, get(model, id).map(|f| f.outer), |l| l.next)
}

/// The edges of a face's loops, in order, each as often as it is used.
pub(crate) fn face_edges(model: &Model, id: Id<Face>) -> Vec<Id<Edge>> {
    let uses = loops(model, id).items().into_iter().filter_map(|l| get(model, l).map(|r| r.first));
    let uses = uses.flat_map(|first| uses_from(model, first).items());
    uses.filter_map(|u| get(model, u).map(|r| r.edge)).collect()
}

/// The faces of the uses of an edge.
pub(crate) fn edge_faces(model: &Model, id: Id<Edge>) -> Vec<Id<Face>> {
    let Some(edge) = get(model, id) else { return Vec::new() };
    let owners = radial(model, edge).items().into_iter().filter_map(|u| get(model, u).map(|r| r.owner));
    let loops = owners.filter_map(|owner| match owner {
        UseOwner::Loop(l) => Some(l),
        UseOwner::Wire(_) => None,
    });
    loops.filter_map(|l| loop_face(model, l)).collect()
}

/// The faces of `targets` that a breadth-first search through shared edges from `start` reaches, keeping to faces
/// `within` allows and stopping once it has reached them all, so that it reads little past the nearest of them.
pub(crate) fn reach(
    model: &Model,
    start: Id<Face>,
    within: impl Fn(Id<Face>) -> bool,
    targets: &BTreeSet<Id<Face>>,
) -> BTreeSet<Id<Face>> {
    let mut seen = BTreeSet::from([start]);
    let mut found: BTreeSet<Id<Face>> = targets.iter().copied().filter(|&t| t == start).collect();
    let mut frontier = VecDeque::from([start]);
    // Each face is searched from once, so no more than the table has slots: a search that has done more stops.
    let (mut searched, most) = (0_usize, Face::table(model).len());
    while let Some(face) = frontier.pop_front().filter(|_| searched < most) {
        searched = searched.saturating_add(1);
        if found.len() == targets.len() {
            break;
        }
        for next in face_edges(model, face).into_iter().flat_map(|e| edge_faces(model, e)) {
            if within(next) && seen.insert(next) {
                if targets.contains(&next) {
                    found.insert(next);
                }
                frontier.push_back(next);
            }
        }
    }
    found
}

/// Every invariant of body `id` as a whole, shell by shell, if its list of shells is sound: each face that names a
/// shell is joined to the face the shell begins from, and a closed shell's faces give a whole genus. A shell that
/// begins from a face not its own is left to its own check.
pub(crate) fn body(model: &Model, members: &Members, id: Id<Body>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let Some(shells) = list(model, record.first_shell, |s| s.next).sound(false).map(<[_]>::to_vec) else { return };
    for shell in shells {
        let joined = faces(model, shell);
        if joined.is_empty() {
            continue;
        }
        let reached: BTreeSet<Id<Face>> = joined.iter().copied().collect();
        let named = members.0.get(&shell).into_iter().flatten();
        if let Some(apart) = named.copied().find(|f| !reached.contains(f)) {
            out.push(failed(Reason::ShellNotConnected, &[shell.entity(), apart.entity()]));
        }
        if record.kind == BodyKind::Solid {
            genus_of(model, shell, &joined, out);
        }
    }
}

/// What a closed shell's Euler–Poincaré formula counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Vertices at the ends of its edges.
    pub vertices: usize,
    /// Edges its loops use.
    pub edges: usize,
    /// Faces joined to the face it begins from.
    pub faces: usize,
    /// Loops of its faces.
    pub loops: usize,
}

/// A count as a whole number that cannot overflow.
fn whole(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl Counts {
    /// `V - E + 2F - L`, which is `2(1 - G)` for a closed shell of genus `G`.
    pub(crate) fn characteristic(self) -> i64 {
        let faces = whole(self.faces).saturating_mul(2);
        whole(self.vertices).saturating_sub(whole(self.edges)).saturating_add(faces).saturating_sub(whole(self.loops))
    }

    /// The counts as values for a report.
    fn values(self) -> [NamedValue; 4] {
        let value = |name, n| NamedValue::new(name, Quantity::Count, f64::from(u32::try_from(n).unwrap_or(u32::MAX)));
        [
            value("vertices", self.vertices),
            value("edges", self.edges),
            value("faces", self.faces),
            value("loops", self.loops),
        ]
    }
}

/// Whether a shell's characteristic gives a whole genus of zero or more.
pub(crate) const fn whole_genus(characteristic: i64) -> bool {
    characteristic & 1 == 0 && characteristic <= 2
}

/// The counts of a shell whose faces are `faces`.
pub(crate) fn counts(model: &Model, faces: &[Id<Face>]) -> Counts {
    let loops: Vec<Id<Loop>> = faces.iter().flat_map(|&f| loops(model, f).items()).collect();
    let uses = loops.iter().filter_map(|&l| get(model, l).map(|r| r.first));
    let edges: BTreeSet<Id<Edge>> =
        uses.flat_map(|first| uses_from(model, first).items()).filter_map(|u| get(model, u).map(|r| r.edge)).collect();
    let ends = edges.iter().filter_map(|&e| get(model, e).map(|r| [r.ends.0, r.ends.1]));
    let vertices: BTreeSet<Id<Vertex>> = ends.flatten().collect();
    Counts { vertices: vertices.len(), edges: edges.len(), faces: faces.len(), loops: loops.len() }
}

/// Reports closed shell `id` if the faces it holds give no whole genus.
pub(crate) fn genus(model: &Model, id: Id<Shell>, out: &mut Vec<Report>) {
    genus_of(model, id, &faces(model, id), out);
}

/// Reports closed shell `id`, whose faces are `faces`, if their counts give no whole genus.
fn genus_of(model: &Model, id: Id<Shell>, faces: &[Id<Face>], out: &mut Vec<Report>) {
    let counts = counts(model, faces);
    if !whole_genus(counts.characteristic()) {
        let report = failed(Reason::GenusNotWhole, &[id.entity()]);
        out.push(counts.values().into_iter().fold(report, Report::with_value));
    }
}
