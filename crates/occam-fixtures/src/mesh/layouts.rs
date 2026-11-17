//! Source: original
//! The tests' models, written through `Edit` as any caller would: sheets of faces, each loop running anticlockwise in
//! its face's parameters, each edge's uses round its ring in the order the faces give them, and each vertex's star in
//! the order the edges give their ends. A flat disc, with its circle drawn in its parameters or not; P9's bicubic patch
//! with a collapsed row; half a sphere, whose loop crosses each pole; a unit square; and that square meeting a quarter
//! of a cylinder along a line, so that one edge is shared by a flat face and a bent one.

#![expect(clippy::arithmetic_side_effects, reason = "small indices in test models")]

use core::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, PI};
use std::sync::Arc;

use occam_fixtures::tol;
use occam_geom::{
    Circle, Curve, Cylinder, Line, ParamCurve, Plane, Sphere, SplineCurve, SplineSurface, Surface, UvCurve,
};
use occam_math::Exec;
use occam_report::StepRole;
use occam_topo::{
    Around, Body, BodyKind, BodyRecord, CurveRecord, Edge, EdgeRecord, EdgeRole, EdgeUse, EdgeUseRecord, Edit, End,
    Face, FaceRecord, Id, Loop, LoopRecord, ParamCurveRecord, Sheet, Shell, ShellRecord, Snapshot, SurfaceRecord,
    UseOwner, Vertex, VertexRecord,
};
use occam_units::{Angle, Direction, Length, Local, Placement, Point, UvPoint, Vec3, Vector, World};

/// An edge: a piece of `curve` over `range`, from point `ends.0` to point `ends.1`.
#[allow(missing_docs, reason = "the sentence above names each field")]
pub struct Piece {
    pub curve: Arc<dyn Curve>,
    pub range: (f64, f64),
    pub ends: (usize, usize),
}

/// A use of edge `edge` in a face's loop, against the edge if `reversed`, with its curve in the face's parameters.
#[allow(missing_docs, reason = "the sentence above names each field")]
pub struct Side {
    pub edge: usize,
    pub reversed: bool,
    pub pcurve: Option<Arc<dyn ParamCurve>>,
}

/// A face on `surface` bounded by one loop of `sides`, with `witness` strictly inside it.
#[allow(missing_docs, reason = "the sentence above names each field")]
pub struct Patch {
    pub surface: Arc<dyn Surface>,
    pub sides: Vec<Side>,
    pub witness: (f64, f64),
}

/// A sheet body: its points, its edges and its faces, all joined into one shell.
#[allow(missing_docs, reason = "the sentence above names each field")]
pub struct Layout {
    pub points: Vec<Vec3>,
    pub edges: Vec<Piece>,
    pub faces: Vec<Patch>,
}

/// The model of `layouts`, each a sheet body of its own, committed; with each one's edges, in its order.
pub fn model(layouts: &[Layout]) -> (Snapshot, Vec<Vec<Id<Edge>>>) {
    let mut edit = Snapshot::empty(tol()).edit();
    let edges = layouts.iter().map(|layout| write(&mut edit, layout)).collect();
    (edit.commit(&Exec::serial()).unwrap().0, edges)
}

/// The item after `item` in `ring`, round to the first after the last.
fn after<T: Copy + PartialEq>(ring: &[T], item: T) -> T {
    let at = ring.iter().position(|&x| x == item).unwrap();
    ring[(at + 1) % ring.len()]
}

/// Writes the faces of `layout`, their loops and their uses, and gives each edge's uses in the order they come.
fn write_faces(
    edit: &mut Edit,
    layout: &Layout,
    shell: Id<Shell>,
    edges: &[Id<Edge>],
) -> (Id<Face>, Vec<Vec<Id<EdgeUse>>>) {
    let born = edit.provenance(StepRole::Made);
    let faces: Vec<Id<Face>> = layout.faces.iter().map(|_| edit.reserve()).collect();
    let loops: Vec<Id<Loop>> = layout.faces.iter().map(|_| edit.reserve()).collect();
    let uses: Vec<Vec<Id<EdgeUse>>> =
        layout.faces.iter().map(|f| f.sides.iter().map(|_| edit.reserve()).collect()).collect();
    let mut rings: Vec<Vec<Id<EdgeUse>>> = vec![Vec::new(); edges.len()];
    for (patch, ids) in layout.faces.iter().zip(&uses) {
        patch.sides.iter().zip(ids).for_each(|(side, &u)| rings[side.edge].push(u));
    }
    for (k, patch) in layout.faces.iter().enumerate() {
        let surface = edit.add(SurfaceRecord(Arc::clone(&patch.surface)));
        let outer = loops[k];
        let face = FaceRecord { shell, surface, sheet: Sheet::default(), reversed: false, outer, born };
        edit.set(faces[k], face).unwrap();
        let witness = UvPoint::new(patch.witness.0, patch.witness.1);
        edit.set(outer, LoopRecord { face: faces[k], first: uses[k][0], next: None, witness }).unwrap();
        for (side, &u) in patch.sides.iter().zip(&uses[k]) {
            let pcurve = side.pcurve.as_ref().map(|p| edit.add(ParamCurveRecord(Arc::clone(p))));
            let (next, radial) = (Some(after(&uses[k], u)), after(&rings[side.edge], u));
            let owner = UseOwner::Loop(outer);
            let record = EdgeUseRecord { edge: edges[side.edge], owner, next, radial, reversed: side.reversed, pcurve };
            edit.set(u, record).unwrap();
        }
    }
    (faces[0], rings)
}

/// Writes `layout` as a sheet body of one shell, and gives its edges.
fn write(edit: &mut Edit, layout: &Layout) -> Vec<Id<Edge>> {
    let born = edit.provenance(StepRole::Made);
    let (body, shell): (Id<Body>, Id<Shell>) = (edit.reserve(), edit.reserve());
    let vertices: Vec<Id<Vertex>> = layout.points.iter().map(|_| edit.reserve()).collect();
    let edges: Vec<Id<Edge>> = layout.edges.iter().map(|_| edit.reserve()).collect();
    let (first_face, rings) = write_faces(edit, layout, shell, &edges);
    let mut stars: Vec<Vec<Around>> = vec![Vec::new(); vertices.len()];
    for (piece, &edge) in layout.edges.iter().zip(&edges) {
        stars[piece.ends.0].push(Around { edge, end: End::Start });
        stars[piece.ends.1].push(Around { edge, end: End::Finish });
    }
    for ((piece, &edge), ring) in layout.edges.iter().zip(&edges).zip(&rings) {
        let curve = edit.add(CurveRecord(Arc::clone(&piece.curve)));
        let (a, b) = piece.ends;
        let around =
            (after(&stars[a], Around { edge, end: End::Start }), after(&stars[b], Around { edge, end: End::Finish }));
        let ends = (vertices[a], vertices[b]);
        let (first_use, role, deviation) = (ring.first().copied(), EdgeRole::Ordinary, Length::from_millimetres(0.0));
        let record = EdgeRecord { curve, range: piece.range, ends, around, first_use, role, deviation, born };
        edit.set(edge, record).unwrap();
    }
    for ((&point, &vertex), star) in layout.points.iter().zip(&vertices).zip(&stars) {
        let record = VertexRecord {
            point: Point::from_vec3(point),
            radius: Length::from_millimetres(0.0),
            star: star.first().copied(),
        };
        edit.set(vertex, record).unwrap();
    }
    edit.set(shell, ShellRecord { body, face: first_face, next: None }).unwrap();
    edit.set(body, BodyRecord { kind: BodyKind::Sheet, first_shell: Some(shell), first_wire: None, born }).unwrap();
    edges
}

/// The straight edge from `points[a]` to `points[b]`, its parameter the distance from `points[a]`.
fn straight(points: &[Vec3], (a, b): (usize, usize)) -> Piece {
    let along = points[b] - points[a];
    let line = Line::new(Point::from_vec3(points[a]), Direction::normalised(along));
    Piece { curve: Arc::new(line), range: (0.0, along.norm()), ends: (a, b) }
}

/// The line in a face's parameters from `(u, v)` along `(du, dv)`, its parameter the distance along it.
pub fn uv_line((u, v): (f64, f64), (du, dv): (f64, f64)) -> Arc<dyn ParamCurve> {
    let line = Line::new(Point::from_vec3(Vec3::new(u, v, 0.0)), Direction::normalised(Vec3::new(du, dv, 0.0)));
    Arc::new(UvCurve::new(line).unwrap())
}

/// A use of `edge`, against it if `reversed`, with its curve in the face's parameters.
fn side(edge: usize, reversed: bool, pcurve: Arc<dyn ParamCurve>) -> Side {
    Side { edge, reversed, pcurve: Some(pcurve) }
}

/// A flat disc of `radius` about the origin, facing +Z: one face bounded by one closed circle, both of whose ends are
/// its one vertex, with no curve in the face's parameters.
pub fn disc(radius: f64) -> Layout {
    let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(radius)).unwrap();
    let edges = vec![Piece { curve: Arc::new(circle), range: (0.0, 2.0 * PI), ends: (0, 0) }];
    let sides = vec![Side { edge: 0, reversed: false, pcurve: None }];
    let faces = vec![Patch { surface: Arc::new(Plane::placed(&Placement::identity())), sides, witness: (0.0, 0.0) }];
    Layout { points: vec![Vec3::new(radius, 0.0, 0.0)], edges, faces }
}

/// A flat triangle in `z = 0`, facing +Z: three straight edges, each drawn in the plane's parameters.
pub fn triangle() -> Layout {
    let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
    let edges = vec![straight(&points, (0, 1)), straight(&points, (1, 2)), straight(&points, (2, 0))];
    let sides = vec![
        side(0, false, uv_line((0.0, 0.0), (1.0, 0.0))),
        side(1, false, uv_line((2.0, 0.0), (-2.0, 1.0))),
        side(2, false, uv_line((0.0, 1.0), (0.0, -1.0))),
    ];
    let faces = vec![Patch { surface: Arc::new(Plane::placed(&Placement::identity())), sides, witness: (0.5, 0.25) }];
    Layout { points, edges, faces }
}

/// [`disc`], with its circle drawn in the plane's parameters, where the plane's `(u, v)` is `(x, y)`.
pub fn disc_drawn(radius: f64) -> Layout {
    let mut layout = disc(radius);
    let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(radius)).unwrap();
    let drawn: Arc<dyn ParamCurve> = Arc::new(UvCurve::new(circle).unwrap());
    layout.faces[0].sides[0].pcurve = Some(drawn);
    layout
}

/// `layout` with the curves of face `face`'s edges in its parameters taken away.
pub fn undrawn(mut layout: Layout, face: usize) -> Layout {
    layout.faces[face].sides.iter_mut().for_each(|side| side.pcurve = None);
    layout
}

/// The poles of P9's bicubic patch, a row of constant `v` at a time: a bump over the unit square whose row at `v = 0`
/// is collapsed to one point.
pub fn collapsed_poles() -> Vec<Point<World>> {
    let height = |i: u32, j: u32| if (1..3).contains(&i) && j > 0 { 0.4 } else { 0.1 * f64::from(i * j) / 9.0 };
    let pole = |i: u32, j: u32| {
        if j == 0 { Vec3::new(0.5, 0.0, 0.0) } else { Vec3::new(f64::from(i) / 3.0, f64::from(j) / 3.0, height(i, j)) }
    };
    (0..4).flat_map(|j| (0..4).map(move |i| Point::from_vec3(pole(i, j)))).collect()
}

/// P9's bicubic patch with a collapsed row: one face bounded by three edges, its sides at `u = 0` and `u = 1` meeting
/// at the point the row at `v = 0` collapses to, where its loop crosses from one side to the other.
pub fn collapsed_patch() -> Layout {
    let poles = collapsed_poles();
    let surface = SplineSurface::bezier((4, 4), poles.clone(), None).unwrap();
    let iso = |at: &[usize]| -> Arc<dyn Curve> {
        Arc::new(SplineCurve::bezier(at.iter().map(|&k| poles[k]).collect(), None).unwrap())
    };
    let piece = |curve, ends| Piece { curve, range: (0.0, 1.0), ends };
    // The sides along `v` at `u = 0` and `u = 1` from the collapsed point, and the row at `v = 1`.
    let edges = vec![
        piece(iso(&[0, 4, 8, 12]), (0, 1)),
        piece(iso(&[12, 13, 14, 15]), (1, 2)),
        piece(iso(&[3, 7, 11, 15]), (0, 2)),
    ];
    let sides = vec![
        side(2, false, uv_line((1.0, 0.0), (0.0, 1.0))),
        side(1, true, uv_line((0.0, 1.0), (1.0, 0.0))),
        side(0, true, uv_line((0.0, 0.0), (0.0, 1.0))),
    ];
    let points = [0, 12, 15].map(|k| poles[k].to_vec3()).into();
    Layout { points, edges, faces: vec![Patch { surface: Arc::new(surface), sides, witness: (0.5, 0.5) }] }
}

/// Half a sphere of `radius` about the origin, as construction lays it out: one face from the meridian at `u = pi` up
/// to the north pole, where its loop crosses to the meridian at `u = 0`, down that to the south pole, and across back.
pub fn sphere_half(radius: f64) -> Layout {
    let turn = |axis: Vec3, angle: f64| {
        Placement::<World, World>::rotation(Direction::normalised(axis), Angle::from_radians(angle))
    };
    let upright = Placement::<Local, World>::identity().then(&turn(Vec3::new(1.0, 0.0, 0.0), FRAC_PI_2));
    let meridian = |frame: &Placement<Local, World>| -> Arc<dyn Curve> {
        Arc::new(Circle::placed(frame, Length::from_millimetres(radius)).unwrap())
    };
    let piece = |curve| Piece { curve, range: (-FRAC_PI_2, FRAC_PI_2), ends: (0, 1) };
    let edges = vec![piece(meridian(&upright)), piece(meridian(&upright.then(&turn(Vec3::new(0.0, 0.0, 1.0), PI))))];
    let sides = vec![side(1, false, uv_line((PI, 0.0), (0.0, 1.0))), side(0, true, uv_line((0.0, 0.0), (0.0, 1.0)))];
    let surface = Arc::new(Sphere::placed(&Placement::identity(), Length::from_millimetres(radius)).unwrap());
    let points = vec![Vec3::new(0.0, 0.0, -radius), Vec3::new(0.0, 0.0, radius)];
    Layout { points, edges, faces: vec![Patch { surface, sides, witness: (FRAC_PI_2, 0.0) }] }
}

/// The frame of the quarter cylinder of [`plane_and_cylinder`]: its axis along +Y through `(0, 0, -radius)`, `u = 0`
/// towards -X and a quarter turn on at the origin, `height` along the axis.
fn cylinder_frame(radius: f64, height: f64) -> Placement<Local, World> {
    let turn = |axis: Vec3, angle: f64| {
        Placement::<World, World>::rotation(Direction::normalised(axis), Angle::from_radians(angle))
    };
    let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, height, -radius)));
    let up = Placement::<Local, World>::identity().then(&turn(Vec3::new(1.0, 0.0, 0.0), FRAC_PI_2));
    up.then(&turn(Vec3::new(0.0, 0.0, 1.0), PI)).then(&shift)
}

/// The unit square in the plane `z = 0`, facing +Z: one face bounded by four straight edges.
pub fn square() -> Layout {
    let points =
        vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
    let edges = [(0, 1), (1, 2), (2, 3), (0, 3)].map(|ends| straight(&points, ends)).into();
    let sides = vec![
        side(0, false, uv_line((0.0, 0.0), (1.0, 0.0))),
        side(1, false, uv_line((1.0, 0.0), (0.0, 1.0))),
        side(2, false, uv_line((1.0, 1.0), (-1.0, 0.0))),
        side(3, true, uv_line((0.0, 0.0), (0.0, 1.0))),
    ];
    let flat = Patch { surface: Arc::new(Plane::placed(&Placement::identity())), sides, witness: (0.5, 0.5) };
    Layout { points, edges, faces: vec![flat] }
}

/// [`square`], and a quarter of a cylinder of `radius` that leaves it along its edge at `x = 0`, tangent to it there,
/// and bends down: a sheet of two faces whose shared edge is straight.
pub fn plane_and_cylinder(radius: f64) -> Layout {
    let mut layout = square();
    layout.points.extend([Vec3::new(-radius, 0.0, -radius), Vec3::new(-radius, 1.0, -radius)]);
    let arc = |height: f64, ends| {
        let circle = Circle::placed(&cylinder_frame(radius, height), Length::from_millimetres(radius)).unwrap();
        Piece { curve: Arc::new(circle), range: (0.0, FRAC_PI_2), ends }
    };
    let bottom = straight(&layout.points, (4, 5));
    layout.edges.extend([arc(0.0, (4, 0)), arc(1.0, (5, 3)), bottom]);
    let bent = vec![
        side(4, false, uv_line((0.0, 0.0), (1.0, 0.0))),
        side(3, false, uv_line((FRAC_PI_2, 0.0), (0.0, 1.0))),
        side(5, true, uv_line((0.0, 1.0), (1.0, 0.0))),
        side(6, true, uv_line((0.0, 0.0), (0.0, 1.0))),
    ];
    let cylinder = Cylinder::placed(&cylinder_frame(radius, 0.0), Length::from_millimetres(radius)).unwrap();
    layout.faces.push(Patch { surface: Arc::new(cylinder), sides: bent, witness: (FRAC_PI_2 / 2.0, 0.5) });
    layout
}

/// A circle of radius 1 about +Z, `height` up it, as an edge from `points[a]` to `points[b]` over the quarter turn from
/// an eighth of a turn, a quarter turn on.
fn arc(height: f64, ends: (usize, usize)) -> Piece {
    let up = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, height)));
    let frame = Placement::<Local, World>::identity().then(&up);
    let circle = Circle::placed(&frame, Length::from_millimetres(1.0)).unwrap();
    Piece { curve: Arc::new(circle), range: (FRAC_PI_4, 3.0 * FRAC_PI_4), ends }
}

/// A flat face under the arc of the unit circle from an eighth of a turn to three eighths, facing +Z: from that arc's
/// ends down to `y = 0`, and from there up to a notch at `x`, `gap` below the arc. At a coarse deflection the arc is
/// one chord, which cuts across the notch. A quarter of a cylinder stands on the arc, down to `z = -1`, so that the arc
/// is an edge of two faces.
pub fn notched(x: f64, gap: f64) -> Layout {
    let (s, top) = (FRAC_1_SQRT_2, (1.0 - x * x).sqrt() - gap);
    let points = vec![
        Vec3::new(-s, 0.0, 0.0),
        Vec3::new(x, top, 0.0),
        Vec3::new(s, 0.0, 0.0),
        Vec3::new(s, s, 0.0),
        Vec3::new(-s, s, 0.0),
        Vec3::new(s, s, -1.0),
        Vec3::new(-s, s, -1.0),
    ];
    let mut edges: Vec<Piece> = [(0, 1), (1, 2), (2, 3)].map(|ends| straight(&points, ends)).into();
    edges.push(arc(0.0, (3, 4)));
    edges.extend([(4, 0)].map(|ends| straight(&points, ends)));
    edges.push(arc(-1.0, (5, 6)));
    edges.extend([(6, 4), (3, 5)].map(|ends| straight(&points, ends)));
    let from = |a: usize, b: usize| {
        let (p, q) = (points[a], points[b]);
        uv_line((p.x, p.y), (q.x - p.x, q.y - p.y))
    };
    let drawn: Arc<dyn ParamCurve> =
        Arc::new(UvCurve::new(Circle::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap()).unwrap());
    let flat = vec![
        side(0, false, from(0, 1)),
        side(1, false, from(1, 2)),
        side(2, false, from(2, 3)),
        side(3, false, drawn),
        side(4, false, from(4, 0)),
    ];
    let (low, high) = (FRAC_PI_4, 3.0 * FRAC_PI_4);
    let wall = vec![
        side(5, false, uv_line((0.0, -1.0), (1.0, 0.0))),
        side(6, false, uv_line((high, -1.0), (0.0, 1.0))),
        side(3, true, uv_line((0.0, 0.0), (1.0, 0.0))),
        side(7, false, uv_line((low, 0.0), (0.0, -1.0))),
    ];
    let cylinder = Cylinder::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap();
    let faces = vec![
        Patch { surface: Arc::new(Plane::placed(&Placement::identity())), sides: flat, witness: (-0.6, 0.3) },
        Patch { surface: Arc::new(cylinder), sides: wall, witness: (FRAC_PI_2, -0.5) },
    ];
    Layout { points, edges, faces }
}

/// A flat lens facing +Z between quarter turns of two circles of radius 1, centred `sqrt(1/2)` below and above the
/// origin: the arc of the upper circle runs along its bottom, and that of the lower along its top. At a deflection of 1
/// each arc is one piece, so its loop joins up to two points.
pub fn lens() -> Layout {
    let s = FRAC_1_SQRT_2;
    let circle = |y: f64| {
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, y, 0.0)));
        Circle::placed(&Placement::<Local, World>::identity().then(&shift), Length::from_millimetres(1.0)).unwrap()
    };
    let (under, over) = (circle(s), circle(-s));
    let edges = vec![
        Piece { curve: Arc::new(under), range: (5.0 * FRAC_PI_4, 7.0 * FRAC_PI_4), ends: (0, 1) },
        Piece { curve: Arc::new(over), range: (FRAC_PI_4, 3.0 * FRAC_PI_4), ends: (1, 0) },
    ];
    let drawn = |circle: Circle| -> Arc<dyn ParamCurve> { Arc::new(UvCurve::new(circle).unwrap()) };
    let sides = vec![side(0, false, drawn(under)), side(1, false, drawn(over))];
    let flat = Patch { surface: Arc::new(Plane::placed(&Placement::identity())), sides, witness: (0.0, 0.0) };
    Layout { points: vec![Vec3::new(-s, 0.0, 0.0), Vec3::new(s, 0.0, 0.0)], edges, faces: vec![flat] }
}
