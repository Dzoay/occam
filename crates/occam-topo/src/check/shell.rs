//! Source: mantyla-1988
//! The invariants of a whole body, shell by shell. Each shell's faces form one list that names it; its faces are joined
//! through their edges, so that two pieces meeting only at a vertex are two shells, the one check that sees a missing
//! pinch (S1 item 6); and a closed shell's counts satisfy the Euler–Poincaré formula `V - E + 2F - L = 2(1 - G)` for a
//! whole genus `G` (Mäntylä 1988), where a vertex where shells meet counts once in each. These read the whole shell;
//! a commit reaches the same answers from what it changed.

use std::collections::{BTreeSet, VecDeque};

use occam_report::{NamedValue, Quantity, Reason, Report};

use super::read::{Walk, get, list, loop_face, radial, uses_from};
use super::{failed, holds};
use crate::kind::sealed::Sealed;
use crate::kind::{Body, Edge, Face, Id, Loop, Shell, Vertex};
use crate::records::{BodyKind, UseOwner};
use crate::snapshot::Model;

/// The faces of a shell's list.
pub(crate) fn faces(model: &Model, id: Id<Shell>) -> Walk<Id<Face>> {
    list(model, get(model, id).map(|s| s.first_face), |f| f.next)
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

/// Every invariant of body `id` as a whole, shell by shell, if its list of shells is sound.
pub(crate) fn body(model: &Model, id: Id<Body>, out: &mut Vec<Report>) {
    let Some(record) = get(model, id) else { return };
    let Some(shells) = list(model, record.first_shell, |s| s.next).sound(false).map(<[_]>::to_vec) else { return };
    for shell in shells {
        let walk = faces(model, shell);
        let named = |f: Id<Face>| get(model, f).map(|r| r.shell) == Some(shell);
        let Some(listed) = holds(out, shell.entity(), &walk, false, Reason::ListBroken, named) else { continue };
        joined(model, shell, &listed, out);
        if record.kind == BodyKind::Solid {
            genus(model, shell, out);
        }
    }
}

/// Reports shell `id` if its `faces` are not all joined through their edges.
fn joined(model: &Model, id: Id<Shell>, faces: &[Id<Face>], out: &mut Vec<Report>) {
    let Some(&start) = faces.first() else { return };
    let targets: BTreeSet<Id<Face>> = faces.iter().copied().collect();
    let found = reach(model, start, |f| targets.contains(&f), &targets);
    if let Some(apart) = targets.difference(&found).next() {
        out.push(failed(Reason::ShellNotConnected, &[id.entity(), apart.entity()]));
    }
}

/// What a closed shell's Euler–Poincaré formula counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Vertices at the ends of its edges.
    pub vertices: usize,
    /// Edges its loops use.
    pub edges: usize,
    /// Faces in its list.
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

/// The counts of shell `id`, walking all of it.
pub(crate) fn counts(model: &Model, id: Id<Shell>) -> Counts {
    let faces = faces(model, id).items();
    let loops: Vec<Id<Loop>> = faces.iter().flat_map(|&f| loops(model, f).items()).collect();
    let uses = loops.iter().filter_map(|&l| get(model, l).map(|r| r.first));
    let edges: BTreeSet<Id<Edge>> =
        uses.flat_map(|first| uses_from(model, first).items()).filter_map(|u| get(model, u).map(|r| r.edge)).collect();
    let ends = edges.iter().filter_map(|&e| get(model, e).map(|r| [r.ends.0, r.ends.1]));
    let vertices: BTreeSet<Id<Vertex>> = ends.flatten().collect();
    Counts { vertices: vertices.len(), edges: edges.len(), faces: faces.len(), loops: loops.len() }
}

/// Reports closed shell `id` if its counts give no whole genus.
pub(crate) fn genus(model: &Model, id: Id<Shell>, out: &mut Vec<Report>) {
    let counts = counts(model, id);
    if !whole_genus(counts.characteristic()) {
        let report = failed(Reason::GenusNotWhole, &[id.entity()]);
        out.push(counts.values().into_iter().fold(report, Report::with_value));
    }
}

/// Reports each face that no shell's list holds.
pub(crate) fn unlisted(model: &Model, out: &mut Vec<Report>) {
    let shells = Shell::table(model).records().map(|(slot, generation, _)| Id::<Shell>::new(slot, generation));
    let listed: BTreeSet<Id<Face>> = shells.flat_map(|s| faces(model, s).items()).collect();
    for (slot, generation, record) in Face::table(model).records() {
        let id = Id::<Face>::new(slot, generation);
        if !listed.contains(&id) {
            out.push(failed(Reason::NotListed, &[id.entity(), record.shell.entity()]));
        }
    }
}
