//! Source: original
//! A body as a primitive or a sweep lays it out, and its writing through the edit: its points, its edges, each a piece
//! of a curve between two of them, and its faces, each a piece of a surface bounded by loops of uses of the edges,
//! every use with its curve in the face's parameters. Faces that share an edge share a shell, so a solid with a void,
//! or a sheet in pieces, is a shell for each piece. Each edge's uses form its ring, and each point's edge ends its
//! star, in the order they are given; `commit` then checks the whole and measures how far each edge strays from its
//! faces. Whether a face's outward normal is its surface's is read from the way its outer loop winds about its witness
//! in the surface's parameters, as the checker counts it, so each loop need only run anticlockwise about the outward
//! normal. What a primitive writes is made by its step from nothing; what a sweep writes is the profile's, each point,
//! edge and face, and the body, derived from what of the profile it came from.

use std::sync::Arc;

use occam_geom::{Curve, ParamCurve, Surface};
use occam_report::{EntityRef, Report, StepRole};
use occam_topo::{
    Around, Body, BodyKind, BodyRecord, CurveRecord, Derivation, Edge, EdgeRecord, EdgeRole, EdgeUse, EdgeUseRecord,
    Edit, End, Face, FaceRecord, Id, Loop, LoopRecord, ParamCurveRecord, Sheet, Shell, ShellRecord, SurfaceRecord,
    UseOwner, Vertex, VertexRecord, winding_number,
};
use occam_units::{Length, Point, UvPoint, World};

/// An edge: a piece of `curve` over `range`, from the point `ends.0` to the point `ends.1`.
pub(crate) struct EdgeSpec {
    pub curve: Arc<dyn Curve>,
    pub range: (f64, f64),
    pub ends: (usize, usize),
    pub role: EdgeRole,
}

/// A use of edge `edge` in a loop, against the edge's direction if `reversed`, with the edge's curve in the face's
/// parameters, which runs over the edge's range in the edge's direction.
pub(crate) struct UseSpec {
    pub edge: usize,
    pub reversed: bool,
    pub pcurve: Arc<dyn ParamCurve>,
}

/// A loop of uses, in order, with a point strictly inside it in the face's parameters.
pub(crate) struct LoopSpec {
    pub uses: Vec<UseSpec>,
    pub witness: UvPoint,
}

/// A face: a piece of `surface` bounded by `loops`, the outer first, each running anticlockwise about its outward
/// normal.
pub(crate) struct FaceSpec {
    pub surface: Arc<dyn Surface>,
    pub loops: Vec<LoopSpec>,
}

/// A closed solid.
pub(crate) struct Solid {
    pub points: Vec<Point<World>>,
    pub edges: Vec<EdgeSpec>,
    pub faces: Vec<FaceSpec>,
}

/// What a body's entities came from: the step's part in making them, the body's kind, and, if they came from entities
/// of the snapshot the edit began from, which ones and how.
pub(crate) struct Origin {
    pub role: StepRole,
    pub kind: BodyKind,
    pub from: Option<Sources>,
}

impl Origin {
    /// A solid made by the step from nothing.
    pub(crate) const MADE: Self = Self { role: StepRole::Made, kind: BodyKind::Solid, from: None };
}

/// What the body, and each point, edge and face in the solid's order, came from, and how.
pub(crate) struct Sources {
    pub how: Derivation,
    pub body: EntityRef,
    pub points: Vec<EntityRef>,
    pub edges: Vec<EntityRef>,
    pub faces: Vec<EntityRef>,
}

/// The ids the writing gives, in the solid's order.
struct Ids {
    body: Id<Body>,
    shells: Vec<Id<Shell>>,
    /// The shell of each face, as its place in `shells`.
    shell_of: Vec<usize>,
    vertices: Vec<Id<Vertex>>,
    edges: Vec<Id<Edge>>,
    faces: Vec<Id<Face>>,
    loops: Vec<Id<Loop>>,
    uses: Vec<Id<EdgeUse>>,
}

/// For each item of the groups, the next item of its group, round in the order given; an item in no group is its own.
fn rings(groups: &[Vec<usize>], count: usize) -> Vec<usize> {
    let mut next: Vec<usize> = (0..count).collect();
    for group in groups {
        for (k, &item) in group.iter().enumerate() {
            let following = group.get(k.saturating_add(1)).or_else(|| group.first());
            if let (Some(slot), Some(&following)) = (next.get_mut(item), following) {
                *slot = following;
            }
        }
    }
    next
}

/// The uses of each edge of the solid, in the order of `uses`, found in one pass over them.
fn by_edge(solid: &Solid, uses: &[(usize, &UseSpec)]) -> Vec<Vec<usize>> {
    let mut by_edge: Vec<Vec<usize>> = vec![Vec::new(); solid.edges.len()];
    for (k, (_, u)) in uses.iter().enumerate() {
        if let Some(members) = by_edge.get_mut(u.edge) {
            members.push(k);
        }
    }
    by_edge
}

/// Every use of the solid, loop by loop and face by face, as its loop's index and its place in that loop.
fn uses_of(solid: &Solid) -> Vec<(usize, &UseSpec)> {
    solid
        .faces
        .iter()
        .flat_map(|f| f.loops.iter())
        .enumerate()
        .flat_map(|(l, lp)| lp.uses.iter().map(move |u| (l, u)))
        .collect()
}

/// Writes `solid` in `edit` as one body of one closed shell, every entity made by this step, and gives the body.
///
/// # Errors
///
/// What the edit reports, which no solid a primitive lays out gives.
pub(crate) fn write(edit: &mut Edit, solid: &Solid) -> Result<Id<Body>, Report> {
    write_from(edit, solid, &Origin::MADE)
}

/// Writes `solid` in `edit` as one body, a shell for each piece of it its edges join, as `origin` says it came about,
/// and gives the body.
///
/// # Errors
///
/// What the edit reports, which no solid a primitive or a sweep lays out gives, and `topo.stale-id` for a source that
/// is not live in the snapshot the edit began from.
pub(crate) fn write_from(edit: &mut Edit, solid: &Solid, origin: &Origin) -> Result<Id<Body>, Report> {
    let uses = uses_of(solid);
    let shells = shells(solid);
    let ids = Ids {
        body: edit.reserve(),
        shells: shells.first.iter().map(|_| edit.reserve()).collect(),
        shell_of: shells.of,
        vertices: solid.points.iter().map(|_| edit.reserve()).collect(),
        edges: solid.edges.iter().map(|_| edit.reserve()).collect(),
        faces: solid.faces.iter().map(|_| edit.reserve()).collect(),
        loops: solid.faces.iter().flat_map(|f| f.loops.iter()).map(|_| edit.reserve()).collect(),
        uses: uses.iter().map(|_| edit.reserve()).collect(),
    };
    write_uses(edit, solid, &ids, &uses)?;
    write_edges(edit, solid, &ids, (&uses, origin.role))?;
    write_faces(edit, solid, &ids, origin.role)?;
    let born = edit.provenance(origin.role);
    for (k, (&shell, &face)) in ids.shells.iter().zip(&shells.first).enumerate() {
        let next = ids.shells.get(k.saturating_add(1)).copied();
        edit.set(shell, ShellRecord { body: ids.body, face: at(&ids.faces, face)?, next })?;
    }
    let body = BodyRecord { kind: origin.kind, first_shell: Some(first(&ids.shells)?), first_wire: None, born };
    edit.set(ids.body, body)?;
    if let Some(from) = &origin.from {
        derive(edit, &ids, from)?;
    }
    Ok(ids.body)
}

/// Records that the body, and each point, edge and face, came from its source in `from`.
fn derive(edit: &mut Edit, ids: &Ids, from: &Sources) -> Result<(), Report> {
    edit.derive(ids.body.entity(), from.body, from.how)?;
    let points = ids.vertices.iter().map(|v| v.entity()).zip(&from.points);
    let edges = ids.edges.iter().map(|e| e.entity()).zip(&from.edges);
    let faces = ids.faces.iter().map(|f| f.entity()).zip(&from.faces);
    for (new, &old) in points.chain(edges).chain(faces) {
        edit.derive(new, old, from.how)?;
    }
    Ok(())
}

/// The shells of a solid's faces: each face's, as a number from 0, and each shell's first face, from which a walk of
/// it begins.
struct Shells {
    of: Vec<usize>,
    first: Vec<usize>,
}

/// The shells of `solid`: faces that share an edge share a shell, each numbered in the order of its first face.
fn shells(solid: &Solid) -> Shells {
    let mut by_edge: Vec<Vec<usize>> = vec![Vec::new(); solid.edges.len()];
    let edges = |face: &FaceSpec| face.loops.iter().flat_map(|l| &l.uses).map(|u| u.edge).collect::<Vec<_>>();
    for (f, face) in solid.faces.iter().enumerate() {
        for e in edges(face) {
            by_edge.get_mut(e).into_iter().for_each(|faces| faces.push(f));
        }
    }
    let (mut of, mut first) = (vec![None; solid.faces.len()], Vec::new());
    for start in 0..solid.faces.len() {
        let mut walk = vec![start];
        while let Some(f) = walk.pop() {
            match of.get_mut(f) {
                Some(slot) if slot.is_none() => *slot = Some(first.len()),
                _ => continue,
            }
            let faces = solid.faces.get(f).map(edges).unwrap_or_default();
            walk.extend(faces.into_iter().flat_map(|e| by_edge.get(e).into_iter().flatten().copied()));
        }
        if of.get(start).copied().flatten() == Some(first.len()) {
            first.push(start);
        }
    }
    Shells { of: of.into_iter().map(|s| s.unwrap_or(0)).collect(), first }
}

/// The first of `ids`, which a solid always has.
fn first<T: Copy>(ids: &[T]) -> Result<T, Report> {
    ids.first().copied().ok_or_else(|| Report::new(occam_report::Code::InvariantFailed, occam_report::Fault::Internal))
}

/// The id at `index`, which the solid's own indices always name.
fn at<T: Copy>(ids: &[T], index: usize) -> Result<T, Report> {
    ids.get(index)
        .copied()
        .ok_or_else(|| Report::new(occam_report::Code::InvariantFailed, occam_report::Fault::Internal))
}

/// Writes every use: its edge, its loop, the next round the loop, the next round its edge, and its curve.
fn write_uses(edit: &mut Edit, solid: &Solid, ids: &Ids, uses: &[(usize, &UseSpec)]) -> Result<(), Report> {
    let by_edge = by_edge(solid, uses);
    let mut by_loop: Vec<Vec<usize>> = vec![Vec::new(); ids.loops.len()];
    for (k, (l, _)) in uses.iter().enumerate() {
        if let Some(members) = by_loop.get_mut(*l) {
            members.push(k);
        }
    }
    let (radial, round) = (rings(&by_edge, uses.len()), rings(&by_loop, uses.len()));
    for (k, (l, u)) in uses.iter().enumerate() {
        let pcurve = edit.add(ParamCurveRecord(Arc::clone(&u.pcurve)));
        let record = EdgeUseRecord {
            edge: at(&ids.edges, u.edge)?,
            owner: UseOwner::Loop(at(&ids.loops, *l)?),
            next: Some(at(&ids.uses, at(&round, k)?)?),
            radial: at(&ids.uses, at(&radial, k)?)?,
            reversed: u.reversed,
            pcurve: Some(pcurve),
        };
        edit.set(at(&ids.uses, k)?, record)?;
    }
    Ok(())
}

/// The place of edge end `n`: edge `n / 2`, at its start where `n` is even.
fn place(ids: &Ids, n: usize) -> Result<Around, Report> {
    let end = if n.is_multiple_of(2) { End::Start } else { End::Finish };
    Ok(Around { edge: at(&ids.edges, n / 2)?, end })
}

/// Writes every edge with its curve, the next place round the star at each end, and its first use; then every vertex,
/// with one place of its star.
fn write_edges(
    edit: &mut Edit,
    solid: &Solid,
    ids: &Ids,
    (uses, role): (&[(usize, &UseSpec)], StepRole),
) -> Result<(), Report> {
    let mut stars: Vec<Vec<usize>> = vec![Vec::new(); solid.points.len()];
    for (k, e) in solid.edges.iter().enumerate() {
        let n = k.saturating_mul(2);
        for (point, n) in [(e.ends.0, n), (e.ends.1, n.saturating_add(1))] {
            if let Some(star) = stars.get_mut(point) {
                star.push(n);
            }
        }
    }
    let round = rings(&stars, solid.edges.len().saturating_mul(2));
    let by_edge = by_edge(solid, uses);
    let born = edit.provenance(role);
    for (k, e) in solid.edges.iter().enumerate() {
        let n = k.saturating_mul(2);
        let curve = edit.add(CurveRecord(Arc::clone(&e.curve)));
        let first = by_edge.get(k).and_then(|members| members.first());
        let first_use = first.map(|&u| at(&ids.uses, u)).transpose()?;
        let record = EdgeRecord {
            curve,
            range: e.range,
            ends: (at(&ids.vertices, e.ends.0)?, at(&ids.vertices, e.ends.1)?),
            around: (place(ids, at(&round, n)?)?, place(ids, at(&round, n.saturating_add(1))?)?),
            first_use,
            role: e.role,
            deviation: Length::from_millimetres(0.0),
            born,
        };
        edit.set(at(&ids.edges, k)?, record)?;
    }
    for (k, &point) in solid.points.iter().enumerate() {
        let star = stars.get(k).and_then(|s| s.first()).map(|&n| place(ids, n)).transpose()?;
        edit.set(at(&ids.vertices, k)?, VertexRecord { point, radius: Length::from_millimetres(0.0), star })?;
    }
    Ok(())
}

/// Writes every face and its loops, each loop with its first use and witness.
fn write_faces(edit: &mut Edit, solid: &Solid, ids: &Ids, role: StepRole) -> Result<(), Report> {
    let born = edit.provenance(role);
    let (mut next_loop, mut next_use) = (0_usize, 0_usize);
    for (k, f) in solid.faces.iter().enumerate() {
        let own = next_loop;
        for (j, lp) in f.loops.iter().enumerate() {
            let following = (j.saturating_add(1) < f.loops.len())
                .then(|| at(&ids.loops, own.saturating_add(j).saturating_add(1)))
                .transpose()?;
            let record = LoopRecord {
                face: at(&ids.faces, k)?,
                first: at(&ids.uses, next_use)?,
                next: following,
                witness: lp.witness,
            };
            edit.set(at(&ids.loops, own.saturating_add(j))?, record)?;
            next_use = next_use.saturating_add(lp.uses.len());
        }
        next_loop = next_loop.saturating_add(f.loops.len());
        let surface = edit.add(SurfaceRecord(Arc::clone(&f.surface)));
        let record = FaceRecord {
            shell: at(&ids.shells, at(&ids.shell_of, k)?)?,
            surface,
            sheet: Sheet::default(),
            reversed: f.loops.first().is_some_and(|outer| clockwise(solid, outer)),
            outer: at(&ids.loops, own)?,
            born,
        };
        edit.set(at(&ids.faces, k)?, record)?;
    }
    Ok(())
}

/// Whether `ring` winds clockwise about its witness in its face's parameters, read along each use at its ends and its
/// middle, the way it runs.
fn clockwise(solid: &Solid, ring: &LoopSpec) -> bool {
    let mut points = Vec::new();
    for used in &ring.uses {
        let Some(edge) = solid.edges.get(used.edge) else { continue };
        let (lo, hi) = edge.range;
        let along = [lo, f64::midpoint(lo, hi), hi].map(|t| used.pcurve.uv(t));
        points.extend(if used.reversed { along.into_iter().rev().collect::<Vec<_>>() } else { along.to_vec() });
    }
    let after = points.iter().skip(1).chain(points.first());
    let segments: Vec<[UvPoint; 2]> = points.iter().zip(after).map(|(&a, &b)| [a, b]).collect();
    winding_number(&segments, ring.witness).is_some_and(i32::is_negative)
}
