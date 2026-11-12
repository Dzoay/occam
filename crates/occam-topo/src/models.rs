//! Source: original
//! Clean models for the tests, built through `Edit` as any caller would: a cube, a block with a square hole through it
//! (genus 1), a cylinder of two patches split at its seam, a square sheet, an open wire, a tiled plate of any size, and
//! two cubes that meet at one corner. Each edge's uses form its
//! radial cycle in the order the faces list them, and each vertex's star runs in the order the edges list their ends.
#![expect(clippy::arithmetic_side_effects, reason = "small indices in test models")]

use core::f64::consts::{FRAC_PI_2, PI};
use std::collections::BTreeMap;
use std::sync::Arc;

use occam_geom::{Line, Plane};
use occam_report::StepRole;
use occam_units::{Angle, Direction, Length, Local, Placement, Point, UvPoint, Vec3, Vector, World};

use crate::edit::Edit;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Loop, Shell, Vertex, Wire};
use crate::records::{
    Around, BodyKind, BodyRecord, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord, Sheet,
    ShellRecord, SurfaceRecord, UseOwner, VertexRecord, WireRecord,
};

mod geometric;

pub(crate) use geometric::{
    cylinder_from, cylinder_pcurves, disc, holed_sheet, sheet_holed_by_its_reversal, straight_pcurves,
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

/// For each of `n` items, each in one of `groups`, the next item of its group, round a cycle in the order given.
fn successors(groups: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut next = vec![0; n];
    for group in groups {
        for (k, &item) in group.iter().enumerate() {
            next[item] = group[(k + 1) % group.len()];
        }
    }
    next
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
    let mut radial: Vec<Vec<usize>> = vec![Vec::new(); edges.len()];
    for (k, u) in specs.iter().enumerate() {
        radial[u.edge].push(k);
    }
    let round = successors(&radial, specs.len());
    for (k, u) in specs.iter().enumerate() {
        let record = EdgeUseRecord {
            edge: edges[u.edge],
            owner: u.owner,
            next: u.next.map(|n| uses[n]),
            radial: uses[round[k]],
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
    // Each place is numbered twice its edge's index, plus one at the finish, and each vertex's star lists its places in
    // the order the edges list their ends.
    let mut stars: Vec<Vec<usize>> = vec![Vec::new(); vertices.len()];
    for (k, e) in spec.edges.iter().enumerate() {
        stars[e.ends.0].push(2 * k);
        stars[e.ends.1].push(2 * k + 1);
    }
    let round = successors(&stars, 2 * spec.edges.len());
    let place =
        |n: usize| Around { edge: edges[n / 2], end: if n.is_multiple_of(2) { End::Start } else { End::Finish } };
    for (k, e) in spec.edges.iter().enumerate() {
        let curve = edit.add(e.curve.clone());
        let next = |end| place(round[2 * k + usize::from(end == End::Finish)]);
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
        let star = stars[k].first().map(|&n| place(n));
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
        edit.set(shell, ShellRecord { body: built.body, face: built.faces[0], next: None }).unwrap();
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
    let mut pairs: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut surfaces = Vec::new();
    let mut specs = Vec::new();
    for &face in faces {
        let mut uses = Vec::new();
        for (k, &a) in face.iter().enumerate() {
            let b = face[(k + 1) % face.len()];
            if let Some(&found) = pairs.get(&(b, a)) {
                uses.push((found, true));
            } else {
                pairs.insert((a, b), edges.len());
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
    cylinder_from(0.0)
}

/// `n` as a coordinate.
fn whole(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap())
}

/// A plate `k` by `k` by 1, its bottom and then its top each tiled by `k²` unit squares, then each side by `k`:
/// `2k² + 4k` faces. Top square `(i, j)` is face `k² + jk + i`.
pub(crate) fn plate(k: usize) -> Spec {
    let n = k + 1;
    let (bottom, top) = (|i: usize, j: usize| j * n + i, |i: usize, j: usize| n * n + j * n + i);
    let corners: Vec<[f64; 3]> =
        [0.0, 1.0].iter().flat_map(|&z| (0..n * n).map(move |c| [whole(c % n), whole(c / n), z])).collect();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let squares = || (0..k).flat_map(move |j| (0..k).map(move |i| (i, j)));
    faces.extend(squares().map(|(i, j)| vec![bottom(i, j), bottom(i, j + 1), bottom(i + 1, j + 1), bottom(i + 1, j)]));
    faces.extend(squares().map(|(i, j)| vec![top(i, j), top(i + 1, j), top(i + 1, j + 1), top(i, j + 1)]));
    for a in 0..k {
        faces.push(vec![bottom(a, 0), bottom(a + 1, 0), top(a + 1, 0), top(a, 0)]);
        faces.push(vec![bottom(a + 1, k), bottom(a, k), top(a, k), top(a + 1, k)]);
        faces.push(vec![bottom(0, a + 1), bottom(0, a), top(0, a), top(0, a + 1)]);
        faces.push(vec![bottom(k, a), bottom(k, a + 1), top(k, a + 1), top(k, a)]);
    }
    let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
    polyhedron(BodyKind::Solid, &corners, &faces)
}

/// A ring one unit square wide round an `m` by `m` square, 1 high, `m` at least 3: its top squares, then its bottom
/// squares, each row `j = 0` first (`i` from 0), then the two squares of each middle row, then row `m - 1`; then its
/// outer walls, four for each `a` in `0..m` (`y = 0`, `y = m`, `x = 0`, `x = m`); then its inner walls, four for each
/// `a` in `1..m - 1` (`y = 1`, `y = m - 1`, `x = 1`, `x = m - 1`). Of genus 1. With `A = 4(m - 1)` squares a side, top
/// square `(c, 0)` is face `c`, the bottom one `A + c`, the outer wall below it `2A + 4c`, and the inner wall above it
/// `2A + 4m + 4(c - 1)`.
pub(crate) fn ring(m: usize) -> Spec {
    let on = |x: usize, y: usize| x <= 1 || y <= 1 || x + 1 >= m || y + 1 >= m;
    let grid: Vec<(usize, usize)> =
        (0..=m).flat_map(|y| (0..=m).map(move |x| (x, y))).filter(|&(x, y)| on(x, y)).collect();
    let index: BTreeMap<(usize, usize), usize> = grid.iter().enumerate().map(|(n, &c)| (c, n)).collect();
    let (b, t) = (|x: usize, y: usize| index[&(x, y)], |x: usize, y: usize| grid.len() + index[&(x, y)]);
    let corners: Vec<[f64; 3]> =
        [0.0, 1.0].iter().flat_map(|&z| grid.iter().map(move |&(x, y)| [whole(x), whole(y), z])).collect();
    let rows = (0..m).flat_map(|j| (0..m).map(move |i| (i, j)));
    let squares: Vec<(usize, usize)> = rows.filter(|&(i, j)| i == 0 || j == 0 || i + 1 == m || j + 1 == m).collect();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    faces.extend(squares.iter().map(|&(i, j)| vec![t(i, j), t(i + 1, j), t(i + 1, j + 1), t(i, j + 1)]));
    faces.extend(squares.iter().map(|&(i, j)| vec![b(i, j), b(i, j + 1), b(i + 1, j + 1), b(i + 1, j)]));
    for a in 0..m {
        faces.push(vec![b(a, 0), b(a + 1, 0), t(a + 1, 0), t(a, 0)]);
        faces.push(vec![b(a + 1, m), b(a, m), t(a, m), t(a + 1, m)]);
        faces.push(vec![b(0, a + 1), b(0, a), t(0, a), t(0, a + 1)]);
        faces.push(vec![b(m, a), b(m, a + 1), t(m, a + 1), t(m, a)]);
    }
    let n = m - 1;
    for a in 1..n {
        faces.push(vec![b(a + 1, 1), b(a, 1), t(a, 1), t(a + 1, 1)]);
        faces.push(vec![b(a, n), b(a + 1, n), t(a + 1, n), t(a, n)]);
        faces.push(vec![b(1, a), b(1, a + 1), t(1, a + 1), t(1, a)]);
        faces.push(vec![b(n, a + 1), b(n, a), t(n, a), t(n, a + 1)]);
    }
    let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
    polyhedron(BodyKind::Solid, &corners, &faces)
}

/// Two unit cubes that meet only at the corner `(1, 1, 1)`, as one body of one shell: the first cube's faces, then the
/// second's.
pub(crate) fn pinched() -> Spec {
    let unit = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
    ];
    let mut corners: Vec<[f64; 3]> = unit.to_vec();
    corners.extend(unit.iter().skip(1).map(|&[x, y, z]| [x + 1., y + 1., z + 1.]));
    // The second cube's corners: its own 0 is the first cube's 6, and its 1 to 7 follow the first cube's eight.
    let second = |c: usize| if c == 0 { 6 } else { 7 + c };
    let quads: [[usize; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [0, 4, 7, 3], [1, 2, 6, 5]];
    let faces: Vec<Vec<usize>> =
        quads.iter().map(|q| q.to_vec()).chain(quads.iter().map(|q| q.iter().map(|&c| second(c)).collect())).collect();
    let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
    polyhedron(BodyKind::Solid, &corners, &faces)
}

/// Joins `place` into the star of `vertex`, if it is live in `edit`, after the place its star names, and gives the place
/// that comes after it.
pub(crate) fn join_star(edit: &mut Edit, vertex: Id<Vertex>, place: Around) -> Around {
    let Some(record) = edit.record(vertex) else { return place };
    let Some(star) = record.star else {
        edit.set(vertex, VertexRecord { star: Some(place), ..record }).unwrap();
        return place;
    };
    let mut before = edit.record(star.edge).unwrap();
    let link = if star.end == End::Start { &mut before.around.0 } else { &mut before.around.1 };
    let after = core::mem::replace(link, place);
    edit.set(star.edge, before).unwrap();
    after
}
