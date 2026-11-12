//! Source: original
//! Clean models for the tests, built through `Edit` as any caller would: a cube, a block with a square hole through it
//! (genus 1), a cylinder of two patches split at its seam, a square sheet and an open wire. Each edge's uses form its
//! radial cycle in the order the faces list them, and each vertex's star runs in the order the edges list their ends.
#![expect(clippy::arithmetic_side_effects, reason = "small indices in test models")]

use core::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use occam_geom::{Circle, Cylinder, Line, Plane};
use occam_report::StepRole;
use occam_units::{Angle, Direction, Length, Local, Placement, Point, UvPoint, Vec3, Vector, World};

use crate::edit::Edit;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Loop, Shell, Vertex, Wire};
use crate::records::{
    Around, BodyKind, BodyRecord, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord, Sheet,
    ShellRecord, SurfaceRecord, UseOwner, VertexRecord, WireRecord,
};

/// An edge: its curve, the part of it, the indices of its start and finish vertices, and its role.
pub(crate) struct EdgeSpec {
    pub curve: CurveRecord,
    pub range: (f64, f64),
    pub ends: (usize, usize),
    pub role: EdgeRole,
}

/// A face: the index of its surface, whether it is reversed, and its loops, the outer first, each a list of edge
/// indices and whether the use runs against the edge, with a witness.
pub(crate) struct FaceSpec {
    pub surface: usize,
    pub reversed: bool,
    pub loops: Vec<(Vec<(usize, bool)>, UvPoint)>,
}

/// A body to build.
pub(crate) struct Spec {
    pub kind: BodyKind,
    pub points: Vec<Point<World>>,
    pub edges: Vec<EdgeSpec>,
    pub surfaces: Vec<SurfaceRecord>,
    pub faces: Vec<FaceSpec>,
    /// Open wires, each a list of edge indices and directions.
    pub wires: Vec<Vec<(usize, bool)>>,
}

/// The ids a build gave, in the spec's order. `uses` lists the faces' loops' uses, then the wires'.
pub(crate) struct Built {
    pub body: Id<Body>,
    pub shell: Option<Id<Shell>>,
    pub faces: Vec<Id<Face>>,
    pub loops: Vec<Id<Loop>>,
    pub wires: Vec<Id<Wire>>,
    pub uses: Vec<Id<EdgeUse>>,
    pub edges: Vec<Id<Edge>>,
    pub vertices: Vec<Id<Vertex>>,
}

/// One use to write: its owner, the edge index, and whether it is reversed.
struct UseSpec {
    owner: UseOwner,
    edge: usize,
    reversed: bool,
    next: Option<usize>,
}

/// The point at these coordinates.
pub(crate) fn point(x: f64, y: f64, z: f64) -> Point<World> {
    Point::from_vec3(Vec3::new(x, y, z))
}

/// Every use, owner by owner, with the index of the next round its loop or along its wire.
fn use_specs(spec: &Spec, loops: &[Id<Loop>], wires: &[Id<Wire>]) -> Vec<UseSpec> {
    let rings = spec
        .faces
        .iter()
        .flat_map(|f| f.loops.iter())
        .zip(loops)
        .map(|((uses, _), &l)| (uses, UseOwner::Loop(l), true));
    let lines = spec.wires.iter().zip(wires).map(|(uses, &w)| (uses, UseOwner::Wire(w), false));
    let mut out = Vec::new();
    for (uses, owner, closed) in rings.chain(lines) {
        let start = out.len();
        for (k, &(edge, reversed)) in uses.iter().enumerate() {
            let following = if k + 1 < uses.len() { Some(start + k + 1) } else { closed.then_some(start) };
            out.push(UseSpec { owner, edge, reversed, next: following });
        }
    }
    out
}

/// For each item of `groups`, the next item of its group, round a cycle in the order given.
fn cycles<T: Copy + PartialEq>(groups: &[Vec<T>], item: T) -> Option<T> {
    let group = groups.iter().find(|g| g.contains(&item))?;
    let at = group.iter().position(|&x| x == item)?;
    group.get((at + 1) % group.len()).copied()
}

/// Builds `spec` in `edit` and gives the ids made.
pub(crate) fn build(edit: &mut Edit, spec: &Spec) -> Built {
    let vertices: Vec<Id<Vertex>> = spec.points.iter().map(|&p| edit.add(vertex_at(p))).collect();
    let surfaces: Vec<_> = spec.surfaces.iter().map(|s| edit.add(s.clone())).collect();
    let edges: Vec<Id<Edge>> = spec.edges.iter().map(|_| edit.reserve()).collect();
    let body = edit.reserve::<Body>();
    let shell = (!spec.faces.is_empty()).then(|| edit.reserve::<Shell>());
    let faces: Vec<Id<Face>> = spec.faces.iter().map(|_| edit.reserve()).collect();
    let loops: Vec<Id<Loop>> = spec.faces.iter().flat_map(|f| f.loops.iter()).map(|_| edit.reserve()).collect();
    let wires: Vec<Id<Wire>> = spec.wires.iter().map(|_| edit.reserve()).collect();
    let specs = use_specs(spec, &loops, &wires);
    let uses: Vec<Id<EdgeUse>> = specs.iter().map(|_| edit.reserve()).collect();
    let radial: Vec<Vec<usize>> =
        (0..edges.len()).map(|e| (0..specs.len()).filter(|&u| specs[u].edge == e).collect()).collect();
    for (k, u) in specs.iter().enumerate() {
        let record = EdgeUseRecord {
            edge: edges[u.edge],
            owner: u.owner,
            next: u.next.map(|n| uses[n]),
            radial: uses[cycles(&radial, k).unwrap()],
            reversed: u.reversed,
            pcurve: None,
        };
        edit.set(uses[k], record).unwrap();
    }
    write_edges(edit, spec, &edges, &vertices, &radial, &uses);
    let built = Built { body, shell, faces, loops, wires, uses, edges, vertices };
    write_faces(edit, spec, &built, &surfaces);
    write_body(edit, spec, &built);
    built
}

/// A given vertex at `p`, its star set later.
fn vertex_at(p: Point<World>) -> VertexRecord {
    VertexRecord { point: p, radius: Length::from_millimetres(0.0), star: None }
}

/// Writes the edges, their curves, and each vertex's star.
fn write_edges(
    edit: &mut Edit,
    spec: &Spec,
    edges: &[Id<Edge>],
    vertices: &[Id<Vertex>],
    radial: &[Vec<usize>],
    uses: &[Id<EdgeUse>],
) {
    let places = |v: usize| -> Vec<Around> {
        let ends = spec.edges.iter().zip(edges).flat_map(|(e, &id)| {
            [(e.ends.0, Around { edge: id, end: End::Start }), (e.ends.1, Around { edge: id, end: End::Finish })]
        });
        ends.filter(|&(at, _)| at == v).map(|(_, place)| place).collect()
    };
    let stars: Vec<Vec<Around>> = (0..vertices.len()).map(places).collect();
    for (k, e) in spec.edges.iter().enumerate() {
        let curve = edit.add(e.curve.clone());
        let next = |end| cycles(&stars, Around { edge: edges[k], end }).unwrap();
        let record = EdgeRecord {
            curve,
            range: e.range,
            ends: (vertices[e.ends.0], vertices[e.ends.1]),
            around: (next(End::Start), next(End::Finish)),
            first_use: radial[k].first().map(|&u| uses[u]),
            role: e.role,
            deviation: Length::from_millimetres(0.0),
            born: edit.provenance(StepRole::Made),
        };
        edit.set(edges[k], record).unwrap();
    }
    for (k, &v) in vertices.iter().enumerate() {
        let star = stars[k].first().copied();
        edit.set(v, VertexRecord { star, ..vertex_at(spec.points[k]) }).unwrap();
    }
}

/// Writes the faces and their loops.
fn write_faces(edit: &mut Edit, spec: &Spec, built: &Built, surfaces: &[Id<crate::kind::SurfaceGeom>]) {
    let mut first_use = 0;
    let mut next_loop = 0;
    for (k, f) in spec.faces.iter().enumerate() {
        let own: Vec<Id<Loop>> = built.loops[next_loop..next_loop + f.loops.len()].to_vec();
        next_loop += f.loops.len();
        for (j, (uses, witness)) in f.loops.iter().enumerate() {
            let record = LoopRecord {
                face: built.faces[k],
                first: built.uses[first_use],
                next: own.get(j + 1).copied(),
                witness: *witness,
            };
            edit.set(own[j], record).unwrap();
            first_use += uses.len();
        }
        let record = FaceRecord {
            shell: built.shell.unwrap(),
            next: built.faces.get(k + 1).copied(),
            surface: surfaces[f.surface],
            sheet: Sheet::default(),
            reversed: f.reversed,
            outer: own[0],
            born: edit.provenance(StepRole::Made),
        };
        edit.set(built.faces[k], record).unwrap();
    }
}

/// Writes the body, its shell and its wires.
fn write_body(edit: &mut Edit, spec: &Spec, built: &Built) {
    if let Some(shell) = built.shell {
        edit.set(shell, ShellRecord { body: built.body, first_face: built.faces[0], next: None }).unwrap();
    }
    let mut first_use: usize = spec.faces.iter().flat_map(|f| f.loops.iter()).map(|(uses, _)| uses.len()).sum();
    for (k, uses) in spec.wires.iter().enumerate() {
        let record =
            WireRecord { body: built.body, first: built.uses[first_use], next: built.wires.get(k + 1).copied() };
        edit.set(built.wires[k], record).unwrap();
        first_use += uses.len();
    }
    let record = BodyRecord {
        kind: spec.kind,
        first_shell: built.shell,
        first_wire: built.wires.first().copied(),
        born: edit.provenance(StepRole::Made),
    };
    edit.set(built.body, record).unwrap();
}

/// The straight edge from `a` to `b`, its parameter the distance from `a`.
fn segment(a: Point<World>, b: Point<World>, ends: (usize, usize)) -> EdgeSpec {
    let d = b.to_vec3() - a.to_vec3();
    let length = d.dot(d).sqrt();
    let curve = CurveRecord(Arc::new(Line::new(a, Direction::normalised(d))));
    EdgeSpec { curve, range: (0.0, length), ends, role: EdgeRole::Ordinary }
}

/// The placement whose XY plane passes through `p` and faces along `normal`, which is along an axis.
fn facing(normal: Vec3, p: Vec3) -> Placement<Local, World> {
    let axis = |x, y, z| Direction::<Local>::normalised(Vec3::new(x, y, z));
    let (about, angle) = if normal.x != 0.0 {
        (axis(0.0, 1.0, 0.0), FRAC_PI_2.copysign(normal.x))
    } else if normal.y != 0.0 {
        (axis(1.0, 0.0, 0.0), -FRAC_PI_2.copysign(normal.y))
    } else {
        (axis(1.0, 0.0, 0.0), if normal.z > 0.0 { 0.0 } else { PI })
    };
    let turn = Placement::<Local, Local>::rotation(about, Angle::from_radians(angle));
    turn.then(&Placement::identity()).then(&Placement::<World, World>::translation(Vector::from_vec3(p)))
}

/// A body of flat faces along the axes, each a list of vertex indices anticlockwise seen from outside, its edges in
/// the order the faces first meet them.
pub(crate) fn polyhedron(kind: BodyKind, corners: &[[f64; 3]], faces: &[&[usize]]) -> Spec {
    let points: Vec<Point<World>> = corners.iter().map(|&[x, y, z]| point(x, y, z)).collect();
    let mut edges: Vec<EdgeSpec> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut surfaces = Vec::new();
    let mut specs = Vec::new();
    for &face in faces {
        let mut uses = Vec::new();
        for (k, &a) in face.iter().enumerate() {
            let b = face[(k + 1) % face.len()];
            if let Some(found) = pairs.iter().position(|&pair| pair == (b, a)) {
                uses.push((found, true));
            } else {
                pairs.push((a, b));
                edges.push(segment(points[a], points[b], (a, b)));
                uses.push((edges.len() - 1, false));
            }
        }
        let vs: Vec<Vec3> = face.iter().map(|&i| points[i].to_vec3()).collect();
        let normal =
            vs.iter().zip(vs.iter().cycle().skip(1)).fold(Vec3::new(0.0, 0.0, 0.0), |n, (&a, &b)| n + a.cross(b));
        let centre = vs.iter().fold(Vec3::new(0.0, 0.0, 0.0), |c, &v| c + v)
            * (1.0 / f64::from(u32::try_from(vs.len()).unwrap()));
        let placement = facing(normal, centre);
        let local = placement.inverse().point(Point::from_vec3(centre)).to_vec3();
        surfaces.push(SurfaceRecord(Arc::new(Plane::placed(&placement))));
        specs.push(FaceSpec {
            surface: surfaces.len() - 1,
            reversed: false,
            loops: vec![(uses, UvPoint::new(local.x, local.y))],
        });
    }
    Spec { kind, points, edges, surfaces, faces: specs, wires: Vec::new() }
}

/// The unit cube, its corner 0 at the origin.
pub(crate) fn cube() -> Spec {
    let corners = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
    ];
    let faces: [&[usize]; 6] =
        [&[0, 3, 2, 1], &[4, 5, 6, 7], &[0, 1, 5, 4], &[2, 3, 7, 6], &[0, 4, 7, 3], &[1, 2, 6, 5]];
    polyhedron(BodyKind::Solid, &corners, &faces)
}

/// A block 3 by 3 by 1 with a 1 by 1 hole through it: a ring of sixteen faces, of genus 1.
pub(crate) fn frame() -> Spec {
    let square = [[0., 0.], [3., 0.], [3., 3.], [0., 3.], [1., 1.], [2., 1.], [2., 2.], [1., 2.]];
    let corners: Vec<[f64; 3]> = [0.0, 1.0].iter().flat_map(|&z| square.iter().map(move |&[x, y]| [x, y, z])).collect();
    let (o, i, top) = (|k: usize| k % 4, |k: usize| 4 + k % 4, 8);
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for k in 0..4 {
        faces.push(vec![top + o(k), top + o(k + 1), top + i(k + 1), top + i(k)]);
        faces.push(vec![o(k), i(k), i(k + 1), o(k + 1)]);
        faces.push(vec![o(k), o(k + 1), top + o(k + 1), top + o(k)]);
        faces.push(vec![i(k + 1), i(k), top + i(k), top + i(k + 1)]);
    }
    let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
    polyhedron(BodyKind::Solid, &corners, &faces)
}

/// A unit square sheet in the XY plane, facing +Z.
pub(crate) fn sheet() -> Spec {
    polyhedron(BodyKind::Sheet, &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]], &[&[0, 1, 2, 3]])
}

/// An open wire of three straight edges.
pub(crate) fn wire() -> Spec {
    let points = vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0), point(1.0, 1.0, 0.0), point(2.0, 1.0, 0.0)];
    let edges = (0..3).map(|k| segment(points[k], points[k + 1], (k, k + 1))).collect();
    let wires = vec![vec![(0, false), (1, false), (2, false)]];
    Spec { kind: BodyKind::Wire, points, edges, surfaces: Vec::new(), faces: Vec::new(), wires }
}

/// A cylinder of radius 1 and height 2 about +Z, its side two patches split at `u = 0` and `u = π`, with flat ends.
pub(crate) fn cylinder() -> Spec {
    let lift = |z: f64| {
        Placement::<Local, World>::identity().then(&Placement::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, z))))
    };
    let radius = Length::from_millimetres(1.0);
    let circle = |z: f64, lo: f64, hi: f64| {
        CurveRecord(Arc::new(Circle::placed(&lift(z), radius).unwrap().trimmed(lo, hi).unwrap()))
    };
    let up =
        |x: f64| CurveRecord(Arc::new(Line::new(point(x, 0.0, 0.0), Direction::normalised(Vec3::new(0.0, 0.0, 1.0)))));
    let edge = |curve, range, ends, role| EdgeSpec { curve, range, ends, role };
    let edges = vec![
        edge(up(1.0), (0.0, 2.0), (0, 2), EdgeRole::Seam),
        edge(up(-1.0), (0.0, 2.0), (1, 3), EdgeRole::Ordinary),
        edge(circle(0.0, 0.0, PI), (0.0, PI), (0, 1), EdgeRole::Ordinary),
        edge(circle(0.0, PI, 2.0 * PI), (PI, 2.0 * PI), (1, 0), EdgeRole::Ordinary),
        edge(circle(2.0, 0.0, PI), (0.0, PI), (2, 3), EdgeRole::Ordinary),
        edge(circle(2.0, PI, 2.0 * PI), (PI, 2.0 * PI), (3, 2), EdgeRole::Ordinary),
    ];
    let side = SurfaceRecord(Arc::new(Cylinder::placed(&Placement::identity(), radius).unwrap()));
    let bottom = SurfaceRecord(Arc::new(Plane::placed(&facing(Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 0.0)))));
    let top = SurfaceRecord(Arc::new(Plane::placed(&lift(2.0))));
    let face = |surface, uses: Vec<(usize, bool)>, u: f64, v: f64| FaceSpec {
        surface,
        reversed: false,
        loops: vec![(uses, UvPoint::new(u, v))],
    };
    let faces = vec![
        face(0, vec![(2, false), (1, false), (4, true), (0, true)], FRAC_PI_2, 1.0),
        face(0, vec![(3, false), (0, false), (5, true), (1, true)], 3.0 * FRAC_PI_2, 1.0),
        face(1, vec![(3, true), (2, true)], 0.0, 0.0),
        face(2, vec![(4, false), (5, false)], 0.0, 0.0),
    ];
    let points = vec![point(1.0, 0.0, 0.0), point(-1.0, 0.0, 0.0), point(1.0, 0.0, 2.0), point(-1.0, 0.0, 2.0)];
    Spec { kind: BodyKind::Solid, points, edges, surfaces: vec![side, bottom, top], faces, wires: Vec::new() }
}
