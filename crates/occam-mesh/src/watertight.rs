//! Source: original
//! The watertight check, run on every mesh, in release builds too: "watertight by construction" covers only the faces
//! that meshed (P9), so it is checked and not assumed. Each shell of each body is checked alone.
//!
//! Vertices are matched by position, never by index: each face's block is its own, and where a face's parameters
//! collapse, at a pole, an apex or a collapsed row, it has two vertices at one position and the triangles either side
//! of the one left out between them meet by position only. Every segment of a shell's mesh must then be used once in
//! each direction, or once along an edge only one face uses, as along a sheet's rim. A face that was not meshed is a
//! hole, named with the edges along which its neighbours are open; a segment used otherwise is named by the edge it
//! lies along, with that edge's faces, or by its face where it lies along none. A shell whose segments all match has
//! the Euler characteristic of its faces, `V - E + 2F - L` (vertices, edges, faces and loops), which its mesh, counted
//! by position, must have too, as `V - E + T`: two places pinched to one position, or a piece too many, show there.

use std::collections::{BTreeMap, BTreeSet};

use occam_report::{Code, EntityRef, Fault, NamedValue, Quantity, Reason, Report};
use occam_topo::{Body, Edge, Face, Id, Shell, Snapshot, Vertex, View};
use occam_units::{Point, World};

use crate::merge::Mesh;

/// A position's bits, by which the check matches vertices.
type Key = [u64; 3];

/// The edge each segment of a face's uses lies along, by the segment's block and its two vertices, the lower first.
type Along = BTreeMap<(usize, u32, u32), Id<Edge>>;

/// One use of a segment by a triangle: its ends' positions, numbered within the shell; the block of the triangle; and
/// the vertices of the mesh it runs between.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Use {
    from: u32,
    to: u32,
    block: usize,
    ends: [u32; 2],
}

/// What the check counts of one shell's mesh: every use of a segment, in order of position, and how many positions and
/// triangles there are.
struct Counted {
    uses: Vec<Use>,
    positions: usize,
    triangles: usize,
}

/// What is wrong with `mesh`, a mesh of `model`, as a watertight mesh: one [`Code::NotWatertight`] report for each face
/// that was not meshed, each edge along which its faces do not meet, each face whose own triangles leave a gap, and each
/// shell whose counts are wrong, shell by shell in the order of the model's bodies; none where it is watertight.
#[must_use]
pub fn watertight(model: &Snapshot, mesh: &Mesh) -> Vec<Report> {
    let blocks: BTreeMap<Id<Face>, usize> = mesh.blocks.iter().enumerate().map(|(k, block)| (block.face, k)).collect();
    let shells = model.ids::<Body>().into_iter().filter_map(|body| model.view(body)).flat_map(View::<Body>::shells);
    shells.flat_map(|shell| check_shell(model, mesh, &blocks, shell)).collect()
}

/// What is wrong with the mesh of `shell`, whose faces' blocks are numbered in `blocks`.
fn check_shell(model: &Snapshot, mesh: &Mesh, blocks: &BTreeMap<Id<Face>, usize>, shell: Id<Shell>) -> Vec<Report> {
    let faces = model.view(shell).map(View::<Shell>::faces).unwrap_or_default();
    let held: Vec<usize> = faces.iter().filter_map(|face| blocks.get(face).copied()).collect();
    let missing: Vec<Id<Face>> = faces.iter().copied().filter(|face| !blocks.contains_key(face)).collect();
    let counted = count(mesh, &held);
    let along = along(mesh, &held);
    let open = unmatched(model, &counted.uses, &along);
    let mut found = named(model, mesh, &missing, &open, &along);
    if found.is_empty() {
        found.extend(characteristic(model, shell, &faces, &counted));
    }
    found
}

/// The bits of `p`.
fn key(p: Point<World>) -> Key {
    let v = p.to_vec3();
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// The position of vertex `k` of `mesh`, if it has one.
fn position(mesh: &Mesh, k: u32) -> Option<Point<World>> {
    usize::try_from(k).ok().and_then(|k| mesh.points.get(k)).copied()
}

/// Every use of a segment by the triangles of `blocks`, sorted, with how many positions and triangles they hold. A
/// triangle with a corner the mesh does not have is left out, and its neighbours are left unmatched.
fn count(mesh: &Mesh, blocks: &[usize]) -> Counted {
    let in_block = |b: usize| {
        let range = mesh.blocks.get(b).map_or(0..0, |block| block.triangles.clone());
        range.filter_map(move |t| usize::try_from(t).ok().and_then(|t| mesh.triangles.get(t)).map(|&t| (b, t)))
    };
    let triangles: Vec<(usize, [u32; 3])> = blocks.iter().flat_map(|&b| in_block(b)).collect();
    let mut keys: Vec<Key> =
        triangles.iter().flat_map(|(_, t)| t.iter().filter_map(|&k| position(mesh, k).map(key))).collect();
    keys.sort_unstable();
    keys.dedup();
    let number = |k: u32| position(mesh, k).and_then(|p| keys.binary_search(&key(p)).ok()?.try_into().ok());
    let mut uses = Vec::new();
    let mut counted = 0_usize;
    for &(block, [i, j, k]) in &triangles {
        let (Some(a), Some(b), Some(c)) = (number(i), number(j), number(k)) else { continue };
        counted = counted.saturating_add(1);
        for (from, to, ends) in [(a, b, [i, j]), (b, c, [j, k]), (c, a, [k, i])] {
            uses.push(Use { from, to, block, ends });
        }
    }
    uses.sort_unstable();
    Counted { uses, positions: keys.len(), triangles: counted }
}

/// The edge each segment of the uses of `blocks` lies along.
fn along(mesh: &Mesh, blocks: &[usize]) -> Along {
    let mut along = Along::new();
    for &b in blocks {
        for side in mesh.blocks.get(b).map_or(&[][..], |block| &block.sides) {
            for pair in side.vertices.windows(2) {
                if let &[p, q] = pair {
                    along.insert((b, p.min(q), p.max(q)), side.edge);
                }
            }
        }
    }
    along
}

/// The edge `u` lies along, if any.
fn edge_of(along: &Along, u: &Use) -> Option<Id<Edge>> {
    let [p, q] = u.ends;
    along.get(&(u.block, p.min(q), p.max(q))).copied()
}

/// The uses in `uses`, sorted, that have no partner: each segment, matched by position, must be used once each way, or
/// once along an edge only one face uses.
fn unmatched(model: &Snapshot, uses: &[Use], along: &Along) -> Vec<Use> {
    let count = |from: u32, to: u32| {
        let start = uses.partition_point(|u| (u.from, u.to) < (from, to));
        uses.partition_point(|u| (u.from, u.to) <= (from, to)).saturating_sub(start)
    };
    let free = |u: &Use| edge_of(along, u).and_then(|edge| model.view(edge)).is_some_and(|edge| edge.uses().len() == 1);
    let matched = |u: &Use| {
        let backward = count(u.to, u.from);
        count(u.from, u.to) == 1 && (backward == 1 || (backward == 0 && free(u)))
    };
    uses.iter().filter(|u| !matched(u)).copied().collect()
}

/// The reports on a shell whose `missing` faces were not meshed and whose `open` uses have no partner: a hole for each
/// face missing, with the edges along which its neighbours are open; then each edge along which the faces that meshed
/// do not meet; then each face whose own triangles leave a gap; each in id order.
fn named(model: &Snapshot, mesh: &Mesh, missing: &[Id<Face>], open: &[Use], along: &Along) -> Vec<Report> {
    let mut holes: BTreeMap<Id<Face>, (BTreeSet<Id<Edge>>, usize)> =
        missing.iter().map(|&face| (face, (BTreeSet::new(), 0))).collect();
    let mut edges: BTreeMap<Id<Edge>, Vec<Use>> = BTreeMap::new();
    let mut inside: BTreeMap<Id<Face>, Vec<Use>> = BTreeMap::new();
    for u in open {
        let Some(edge) = edge_of(along, u) else {
            if let Some(block) = mesh.blocks.get(u.block) {
                inside.entry(block.face).or_default().push(*u);
            }
            continue;
        };
        let faces = model.view(edge).map(View::<Edge>::faces).unwrap_or_default();
        let missed = faces.into_iter().find(|face| holes.contains_key(face));
        match missed.and_then(|face| holes.get_mut(&face)) {
            Some((rim, unmatched)) => {
                rim.insert(edge);
                *unmatched = unmatched.saturating_add(1);
            }
            None => edges.entry(edge).or_default().push(*u),
        }
    }
    let mut found: Vec<Report> = holes.into_iter().map(|(face, (rim, n))| hole(mesh, face, &rim, n)).collect();
    for (edge, uses) in edges {
        let faces = model.view(edge).map(View::<Edge>::faces).unwrap_or_default();
        let named: Vec<EntityRef> = std::iter::once(edge.entity()).chain(faces.into_iter().map(Id::entity)).collect();
        found.push(crack(mesh, &named, &uses));
    }
    found.extend(inside.into_iter().map(|(face, uses)| crack(mesh, &[face.entity()], &uses)));
    found
}

/// A count as a report's value.
fn counted(name: &str, n: usize) -> NamedValue {
    NamedValue::count(name, u32::try_from(n).unwrap_or(u32::MAX))
}

/// The hole where `face` was not meshed, open along the edges of `rim` by `unmatched` uses, with the fault of the
/// report refusing the face.
fn hole(mesh: &Mesh, face: Id<Face>, rim: &BTreeSet<Id<Edge>>, unmatched: usize) -> Report {
    let refusal = mesh.refused.iter().find(|report| report.entities.first() == Some(&face.entity()));
    let fault = refusal.and_then(|report| report.fault).unwrap_or(Fault::Internal);
    let report = Report::new(Code::NotWatertight, fault).with_reason(Reason::FaceMissing).with_entity(face.entity());
    rim.iter()
        .fold(report, |report, edge| report.with_entity(edge.entity()))
        .with_value(counted("unmatched", unmatched))
}

/// The report on `uses`, which have no partner, naming `named`, with how many there are and where the first runs.
fn crack(mesh: &Mesh, named: &[EntityRef], uses: &[Use]) -> Report {
    let report = Report::new(Code::NotWatertight, Fault::Internal).with_reason(Reason::SegmentsUnmatched);
    let report = named.iter().fold(report, |report, &entity| report.with_entity(entity));
    let report = report.with_value(counted("unmatched", uses.len()));
    let at = |k: u32| position(mesh, k).map(|p| [p.to_vec3().x, p.to_vec3().y, p.to_vec3().z]);
    let Some([Some(from), Some(to)]) = uses.first().map(|u| u.ends.map(at)) else { return report };
    report.with_value(NamedValue::point("from", from)).with_value(NamedValue::point("to", to))
}

/// A count as a whole number that cannot overflow.
fn whole(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// `V - E + 2F - L` of `faces`: their vertices, edges, faces and loops.
fn of_faces(model: &Snapshot, faces: &[Id<Face>]) -> i64 {
    let (mut loops, mut edges) = (0_usize, BTreeSet::new());
    for face in faces.iter().filter_map(|&face| model.view(face)) {
        loops = loops.saturating_add(face.loops().len());
        edges.extend(face.edges());
    }
    let ends = edges.iter().filter_map(|&edge| model.view(edge)).flat_map(|edge| <[Id<Vertex>; 2]>::from(edge.ends()));
    let vertices: BTreeSet<Id<Vertex>> = ends.collect();
    let faces = whole(faces.len()).saturating_mul(2);
    whole(vertices.len()).saturating_sub(whole(edges.len())).saturating_add(faces).saturating_sub(whole(loops))
}

/// The report on `shell`, whose faces are `faces`, if its mesh, counted by position, has another Euler characteristic.
fn characteristic(model: &Snapshot, shell: Id<Shell>, faces: &[Id<Face>], counted: &Counted) -> Option<Report> {
    let segments: BTreeSet<(u32, u32)> = counted.uses.iter().map(|u| (u.from.min(u.to), u.from.max(u.to))).collect();
    let found = whole(counted.positions).saturating_sub(whole(segments.len())).saturating_add(whole(counted.triangles));
    let expected = of_faces(model, faces);
    let value =
        |name: &str, n: i64| NamedValue::new(name, Quantity::Count, f64::from(i32::try_from(n).unwrap_or(i32::MAX)));
    (found != expected).then(|| {
        let report = Report::new(Code::NotWatertight, Fault::Internal).with_reason(Reason::CharacteristicDiffers);
        report.with_entity(shell.entity()).with_value(value("found", found)).with_value(value("expected", expected))
    })
}

#[cfg(test)]
mod tests;
