//! Source: original
//! A solid written through `Edit` as any caller would, for the parts no request can make yet: points, edges and faces
//! given by index, each face's loops with a curve in its parameters on every use. Each edge's uses form its radial
//! cycle in the order the faces give them, and each vertex's star runs in the order the edges give their ends.

use std::sync::Arc;

use occam_geom::{Curve, Line, ParamCurve, Surface, UvCurve};
use occam_report::{Code, Fault, Report, StepRole};
use occam_topo::{
    Around, Body, BodyKind, BodyRecord, CurveRecord, Edge, EdgeRecord, EdgeRole, EdgeUse, EdgeUseRecord, Edit, End,
    Face, FaceRecord, Id, Loop, LoopRecord, ParamCurveRecord, Sheet, Shell, ShellRecord, SurfaceRecord, UseOwner,
    Vertex, VertexRecord,
};
use occam_units::{Direction, Length, Point, UvPoint, Vec3, World};

/// An edge: a piece of `curve` over `range`, from point `ends.0` to point `ends.1`. How far it strays from its faces
/// is left to the commit, which measures it.
pub(crate) struct Piece {
    pub curve: Arc<dyn Curve>,
    pub range: (f64, f64),
    pub ends: (usize, usize),
}

/// A use of edge `edge`, against it if `reversed`, with its curve in the face's parameters.
pub(crate) struct Use {
    pub edge: usize,
    pub reversed: bool,
    pub pcurve: Arc<dyn ParamCurve>,
}

/// A loop of uses, with a point strictly inside it.
pub(crate) struct Ring {
    pub uses: Vec<Use>,
    pub witness: UvPoint,
}

/// A face on `surface`, its outward normal the surface's reversed if `reversed`, its outer loop first.
pub(crate) struct Sided {
    pub surface: Arc<dyn Surface>,
    pub reversed: bool,
    pub loops: Vec<Ring>,
}

/// A solid of one shell.
#[derive(Default)]
pub(crate) struct Solid {
    pub points: Vec<Point<World>>,
    pub edges: Vec<Piece>,
    pub faces: Vec<Sided>,
}

/// The report of a workload that names a point, an edge or a use it does not have: a fault in the workload.
pub(crate) fn missing() -> Report {
    Report::new(Code::WrongCount, Fault::Internal)
}

/// The item at `k` of `list`.
fn at<T: Copy>(list: &[T], k: usize) -> Result<T, Report> {
    list.get(k).copied().ok_or_else(missing)
}

/// The item after `item` in `ring`, round to the first after the last.
fn after<T: Copy + PartialEq>(ring: &[T], item: T) -> Result<T, Report> {
    let place = ring.iter().position(|&x| x == item).ok_or_else(missing)?;
    at(ring, place.checked_add(1).filter(|&next| next < ring.len()).unwrap_or(0))
}

/// The line in a face's parameters from `(u, v)` along `(du, dv)`, its parameter the distance along it.
pub(crate) fn uv_line((u, v): (f64, f64), (du, dv): (f64, f64)) -> Result<Arc<dyn ParamCurve>, Report> {
    let line = Line::new(Point::from_vec3(Vec3::new(u, v, 0.0)), Direction::normalised(Vec3::new(du, dv, 0.0)));
    Ok(Arc::new(UvCurve::new(line)?))
}

/// A face on `surface` over the rectangle `u.0..u.1` by `v.0..v.1` of its parameters, bounded by the edges along its
/// sides, `[bottom, right, top, left]`: each edge's parameter is the surface's along its side, rising. The loop runs
/// anticlockwise, or clockwise for a face that is `reversed`.
pub(crate) fn quad(
    surface: Arc<dyn Surface>,
    reversed: bool,
    (u, v): ((f64, f64), (f64, f64)),
    [bottom, right, top, left]: [usize; 4],
) -> Result<Sided, Report> {
    let side = |edge, against, pcurve| Ok::<_, Report>(Use { edge, reversed: against, pcurve });
    let mut uses = vec![
        side(bottom, false, uv_line((0.0, v.0), (1.0, 0.0))?)?,
        side(right, false, uv_line((u.1, 0.0), (0.0, 1.0))?)?,
        side(top, true, uv_line((0.0, v.1), (1.0, 0.0))?)?,
        side(left, true, uv_line((u.0, 0.0), (0.0, 1.0))?)?,
    ];
    if reversed {
        uses.reverse();
        for one in &mut uses {
            one.reversed = !one.reversed;
        }
    }
    let witness = UvPoint::new(f64::midpoint(u.0, u.1), f64::midpoint(v.0, v.1));
    Ok(Sided { surface, reversed, loops: vec![Ring { uses, witness }] })
}

/// The ids reserved for a solid's loops and uses, face by face and loop by loop.
struct Reserved {
    faces: Vec<Id<Face>>,
    loops: Vec<Vec<Id<Loop>>>,
    uses: Vec<Vec<Vec<Id<EdgeUse>>>>,
}

fn reserve(edit: &mut Edit, solid: &Solid) -> Reserved {
    let faces = solid.faces.iter().map(|_| edit.reserve()).collect();
    let loops = solid.faces.iter().map(|face| face.loops.iter().map(|_| edit.reserve()).collect()).collect();
    let uses = solid
        .faces
        .iter()
        .map(|face| face.loops.iter().map(|ring| ring.uses.iter().map(|_| edit.reserve()).collect()).collect())
        .collect();
    Reserved { faces, loops, uses }
}

/// Each edge's uses, in the order the faces give them.
fn rings(solid: &Solid, ids: &Reserved) -> Vec<Vec<Id<EdgeUse>>> {
    let mut rings = vec![Vec::new(); solid.edges.len()];
    let given = solid.faces.iter().flat_map(|face| face.loops.iter()).flat_map(|ring| ring.uses.iter());
    let reserved = ids.uses.iter().flatten().flatten();
    for (one, &id) in given.zip(reserved) {
        if let Some(ring) = rings.get_mut(one.edge) {
            ring.push(id);
        }
    }
    rings
}

/// Writes the faces of `solid`, their loops and their uses.
fn write_faces(
    edit: &mut Edit,
    solid: &Solid,
    (shell, edges): (Id<Shell>, &[Id<Edge>]),
    ids: &Reserved,
    rings: &[Vec<Id<EdgeUse>>],
) -> Result<(), Report> {
    let born = edit.provenance(StepRole::Made);
    for (((sided, &face), loops), uses) in solid.faces.iter().zip(&ids.faces).zip(&ids.loops).zip(&ids.uses) {
        let surface = edit.add(SurfaceRecord(Arc::clone(&sided.surface)));
        let outer = at(loops, 0)?;
        let record = FaceRecord { shell, surface, sheet: Sheet::default(), reversed: sided.reversed, outer, born };
        edit.set(face, record)?;
        for (k, ((ring, &id), ring_uses)) in sided.loops.iter().zip(loops).zip(uses).enumerate() {
            let next = loops.get(k.saturating_add(1)).copied();
            let first = at(ring_uses, 0)?;
            edit.set(id, LoopRecord { face, first, next, witness: ring.witness })?;
            for (one, &u) in ring.uses.iter().zip(ring_uses) {
                let pcurve = Some(edit.add(ParamCurveRecord(Arc::clone(&one.pcurve))));
                let edge = at(edges, one.edge)?;
                let radial = after(rings.get(one.edge).ok_or_else(missing)?, u)?;
                let (owner, next) = (UseOwner::Loop(id), Some(after(ring_uses, u)?));
                edit.set(u, EdgeUseRecord { edge, owner, next, radial, reversed: one.reversed, pcurve })?;
            }
        }
    }
    Ok(())
}

/// Writes the edges of `solid` and its vertices, each vertex's star in the order the edges give their ends.
fn write_edges(
    edit: &mut Edit,
    solid: &Solid,
    (edges, vertices): (&[Id<Edge>], &[Id<Vertex>]),
    rings: &[Vec<Id<EdgeUse>>],
) -> Result<(), Report> {
    let born = edit.provenance(StepRole::Made);
    let mut stars: Vec<Vec<Around>> = vec![Vec::new(); vertices.len()];
    for (piece, &edge) in solid.edges.iter().zip(edges) {
        stars.get_mut(piece.ends.0).ok_or_else(missing)?.push(Around { edge, end: End::Start });
        stars.get_mut(piece.ends.1).ok_or_else(missing)?.push(Around { edge, end: End::Finish });
    }
    for ((piece, &edge), ring) in solid.edges.iter().zip(edges).zip(rings) {
        let curve = edit.add(CurveRecord(Arc::clone(&piece.curve)));
        let (a, b) = piece.ends;
        let star = |k: usize| stars.get(k).ok_or_else(missing);
        let around =
            (after(star(a)?, Around { edge, end: End::Start })?, after(star(b)?, Around { edge, end: End::Finish })?);
        let (first_use, role) = (ring.first().copied(), EdgeRole::Ordinary);
        let ends = (at(vertices, a)?, at(vertices, b)?);
        let deviation = Length::from_millimetres(0.0);
        edit.set(edge, EdgeRecord { curve, range: piece.range, ends, around, first_use, role, deviation, born })?;
    }
    for ((&point, &vertex), star) in solid.points.iter().zip(vertices).zip(&stars) {
        let radius = Length::from_millimetres(0.0);
        edit.set(vertex, VertexRecord { point, radius, star: star.first().copied() })?;
    }
    Ok(())
}

/// Writes `solid` in `edit` as a solid body of one shell.
pub(crate) fn write(edit: &mut Edit, solid: &Solid) -> Result<(), Report> {
    let born = edit.provenance(StepRole::Made);
    let (body, shell): (Id<Body>, Id<Shell>) = (edit.reserve(), edit.reserve());
    let vertices: Vec<Id<Vertex>> = solid.points.iter().map(|_| edit.reserve()).collect();
    let edges: Vec<Id<Edge>> = solid.edges.iter().map(|_| edit.reserve()).collect();
    let ids = reserve(edit, solid);
    let rings = rings(solid, &ids);
    write_faces(edit, solid, (shell, &edges), &ids, &rings)?;
    write_edges(edit, solid, (&edges, &vertices), &rings)?;
    edit.set(shell, ShellRecord { body, face: at(&ids.faces, 0)?, next: None })?;
    edit.set(body, BodyRecord { kind: BodyKind::Solid, first_shell: Some(shell), first_wire: None, born })?;
    Ok(())
}
