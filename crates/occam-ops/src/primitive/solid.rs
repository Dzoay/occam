//! Source: original
//! A closed solid as a primitive lays it out, and its writing through the edit: its points, its edges, each a piece of
//! a curve between two of them, and its faces, each a piece of a surface bounded by loops of uses of the edges, every
//! use with its curve in the face's parameters. Each edge's uses form its ring, and each point's edge ends its star, in
//! the order they are given; `commit` then checks the whole and measures how far each edge strays from its faces.

use std::sync::Arc;

use occam_geom::{Curve, ParamCurve, Surface};
use occam_report::{Report, StepRole};
use occam_topo::{
    Around, Body, BodyKind, BodyRecord, CurveRecord, Edge, EdgeRecord, EdgeRole, EdgeUse, EdgeUseRecord, Edit, End,
    Face, FaceRecord, Id, Loop, LoopRecord, ParamCurveRecord, Sheet, Shell, ShellRecord, SurfaceRecord, UseOwner,
    Vertex, VertexRecord,
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

/// A face: a piece of `surface`, whose normal points out of the solid, bounded by `loops`, the outer first.
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

/// The ids the writing gives, in the solid's order.
struct Ids {
    body: Id<Body>,
    shell: Id<Shell>,
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
    let uses = uses_of(solid);
    let ids = Ids {
        body: edit.reserve(),
        shell: edit.reserve(),
        vertices: solid.points.iter().map(|_| edit.reserve()).collect(),
        edges: solid.edges.iter().map(|_| edit.reserve()).collect(),
        faces: solid.faces.iter().map(|_| edit.reserve()).collect(),
        loops: solid.faces.iter().flat_map(|f| f.loops.iter()).map(|_| edit.reserve()).collect(),
        uses: uses.iter().map(|_| edit.reserve()).collect(),
    };
    write_uses(edit, solid, &ids, &uses)?;
    write_edges(edit, solid, &ids, &uses)?;
    write_faces(edit, solid, &ids)?;
    let born = edit.provenance(StepRole::Made);
    edit.set(ids.shell, ShellRecord { body: ids.body, face: first(&ids.faces)?, next: None })?;
    let body = BodyRecord { kind: BodyKind::Solid, first_shell: Some(ids.shell), first_wire: None, born };
    edit.set(ids.body, body)?;
    Ok(ids.body)
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
    let by_edge: Vec<Vec<usize>> = (0..solid.edges.len())
        .map(|e| uses.iter().enumerate().filter(|(_, (_, u))| u.edge == e).map(|(k, _)| k).collect())
        .collect();
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
fn write_edges(edit: &mut Edit, solid: &Solid, ids: &Ids, uses: &[(usize, &UseSpec)]) -> Result<(), Report> {
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
    let born = edit.provenance(StepRole::Made);
    for (k, e) in solid.edges.iter().enumerate() {
        let n = k.saturating_mul(2);
        let curve = edit.add(CurveRecord(Arc::clone(&e.curve)));
        let first_use = uses.iter().position(|(_, u)| u.edge == k).map(|u| at(&ids.uses, u)).transpose()?;
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
fn write_faces(edit: &mut Edit, solid: &Solid, ids: &Ids) -> Result<(), Report> {
    let born = edit.provenance(StepRole::Made);
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
            shell: ids.shell,
            surface,
            sheet: Sheet::default(),
            reversed: false,
            outer: at(&ids.loops, own)?,
            born,
        };
        edit.set(at(&ids.faces, k)?, record)?;
    }
    Ok(())
}
