//! Source: original
//! Clean models for the checks that read geometry, and what they add to a model: parameter curves, a cylinder split at
//! any angle, a disc bounded by one closed edge, and a sheet with a hole, true or its own reversal.
#![expect(clippy::arithmetic_side_effects, reason = "small indices in test models")]

use core::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use occam_geom::{Circle, Curve, Cylinder, Line, Plane, Scratch, Surface, UvCurve};
use occam_math::sin_cos;
use occam_units::{Direction, Length, Local, Placement, Point, UvPoint, Vec3, Vector, World};

use super::{Built, EdgeSpec, FaceSpec, Spec, facing, point, segment, sheet};
use crate::edit::Edit;
use crate::records::{BodyKind, CurveRecord, EdgeRole, EdgeUseRecord, ParamCurveRecord, SurfaceRecord, UseOwner};
use crate::testing::tolerance;

/// The straight line in a face's parameters from `(u, v)`, along `(du, dv)`, its parameter the distance along it.
pub(crate) fn uv_line((u, v): (f64, f64), (du, dv): (f64, f64)) -> ParamCurveRecord {
    let line = Line::new(point(u, v, 0.0), Direction::normalised(Vec3::new(du, dv, 0.0)));
    ParamCurveRecord(Arc::new(UvCurve::new(line).unwrap()))
}

/// The straight line in `surface`'s parameters through the images of `curve`'s points at the ends of `range`, its
/// parameter running over `range`: the parameter curve of a straight edge on a plane, whose parameters are lengths.
pub(crate) fn straight(surface: &dyn Surface, curve: &dyn Curve, (lo, hi): (f64, f64)) -> ParamCurveRecord {
    let image = |t| surface.invert(curve.point(t), None, tolerance(), &mut Scratch::default()).unwrap();
    let (a, b) = (image(lo), image(hi));
    let d = Vec3::new(b.u() - a.u(), b.v() - a.v(), 0.0);
    let start = Vec3::new(a.u(), a.v(), 0.0) - d * (lo / d.norm());
    uv_line((start.x, start.y), (d.x, d.y))
}

/// Gives each use of a face's loop in `built` the parameter curve [`straight`] makes, so a body of straight edges on
/// planes, as [`super::polyhedron`] builds, has one for every use.
pub(crate) fn straight_pcurves(edit: &mut Edit, built: &Built) {
    for &u in &built.uses {
        let record = edit.record(u).unwrap();
        let UseOwner::Loop(l) = record.owner else { continue };
        let face = edit.record(edit.record(l).unwrap().face).unwrap();
        let surface = edit.record(face.surface).unwrap().0;
        let edge = edit.record(record.edge).unwrap();
        let curve = edit.record(edge.curve).unwrap().0;
        let pcurve = edit.add(straight(&*surface, &*curve, edge.range));
        edit.set(u, EdgeUseRecord { pcurve: Some(pcurve), ..record }).unwrap();
    }
}

/// A cylinder of radius 1 and height 2 about +Z, its side two patches split at `u = start` and `u = start + π`, with the
/// seam edge at `start`, and flat ends. From `start` above 0 the second patch runs past `u = 2π`, across the surface's
/// own seam.
pub(crate) fn cylinder_from(start: f64) -> Spec {
    let (a, b, c) = (start, start + PI, start + 2.0 * PI);
    let lift = |z: f64| {
        Placement::<Local, World>::identity().then(&Placement::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, z))))
    };
    let radius = Length::from_millimetres(1.0);
    let at = |angle: f64, z: f64| {
        let (sin, cos) = sin_cos(angle);
        point(cos, sin, z)
    };
    let circle = |z: f64, lo: f64, hi: f64| {
        CurveRecord(Arc::new(Circle::placed(&lift(z), radius).unwrap().trimmed(lo, hi).unwrap()))
    };
    let up =
        |angle: f64| CurveRecord(Arc::new(Line::new(at(angle, 0.0), Direction::normalised(Vec3::new(0., 0., 1.)))));
    let edge = |curve, range, ends, role| EdgeSpec { curve, range, ends, role };
    let edges = vec![
        edge(up(a), (0.0, 2.0), (0, 2), EdgeRole::Seam),
        edge(up(b), (0.0, 2.0), (1, 3), EdgeRole::Ordinary),
        edge(circle(0.0, a, b), (a, b), (0, 1), EdgeRole::Ordinary),
        edge(circle(0.0, b, c), (b, c), (1, 0), EdgeRole::Ordinary),
        edge(circle(2.0, a, b), (a, b), (2, 3), EdgeRole::Ordinary),
        edge(circle(2.0, b, c), (b, c), (3, 2), EdgeRole::Ordinary),
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
        face(0, vec![(2, false), (1, false), (4, true), (0, true)], a + FRAC_PI_2, 1.0),
        face(0, vec![(3, false), (0, false), (5, true), (1, true)], b + FRAC_PI_2, 1.0),
        face(1, vec![(3, true), (2, true)], 0.0, 0.0),
        face(2, vec![(4, false), (5, false)], 0.0, 0.0),
    ];
    let points = vec![at(a, 0.0), at(b, 0.0), at(a, 2.0), at(b, 2.0)];
    Spec { kind: BodyKind::Solid, points, edges, surfaces: vec![side, bottom, top], faces, wires: Vec::new() }
}

/// Gives the uses of the side of a cylinder from [`cylinder_from`] their parameter curves, `shift` further on in `u`:
/// each straight edge at its angle, the seam at `start` on the first patch and a turn further on the second, and each
/// circle along its height.
pub(crate) fn cylinder_pcurves(edit: &mut Edit, built: &Built, start: f64, shift: f64) {
    let (a, b, c) = (start + shift, start + PI + shift, start + 2.0 * PI + shift);
    let patches = [
        (0, [(2, None), (1, Some(b)), (4, None), (0, Some(a))]),
        (4, [(3, None), (0, Some(c)), (5, None), (1, Some(b))]),
    ];
    for (first, uses) in patches {
        for (k, (edge, at)) in uses.into_iter().enumerate() {
            let height = if edge < 4 { 0.0 } else { 2.0 };
            let pcurve = at.map_or_else(|| uv_line((shift, height), (1.0, 0.0)), |u| uv_line((u, 0.0), (0.0, 1.0)));
            let pcurve = edit.add(pcurve);
            let id = built.uses[first + k];
            let record = edit.record(id).unwrap();
            edit.set(id, EdgeUseRecord { pcurve: Some(pcurve), ..record }).unwrap();
        }
    }
}

/// A flat disc of radius 1 about the origin, facing +Z: one face bounded by one closed circular edge, whose two ends
/// are one vertex.
pub(crate) fn disc() -> Spec {
    let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap();
    let edges = vec![EdgeSpec {
        curve: CurveRecord(Arc::new(circle)),
        range: (0.0, 2.0 * PI),
        ends: (0, 0),
        role: EdgeRole::Ordinary,
    }];
    let plane = SurfaceRecord(Arc::new(Plane::placed(&Placement::identity())));
    let faces = vec![FaceSpec { surface: 0, reversed: false, loops: vec![(vec![(0, false)], UvPoint::new(0.0, 0.0))] }];
    Spec {
        kind: BodyKind::Sheet,
        points: vec![point(1.0, 0.0, 0.0)],
        edges,
        surfaces: vec![plane],
        faces,
        wires: Vec::new(),
    }
}

/// A square sheet 3 by 3 in the XY plane, facing +Z, with a square hole 1 by 1 in its middle: the outer loop runs
/// anticlockwise with its witness in the sheet, and the hole clockwise with its witness in the hole.
pub(crate) fn holed_sheet() -> Spec {
    let corners = [[0., 0.], [3., 0.], [3., 3.], [0., 3.], [1., 1.], [1., 2.], [2., 2.], [2., 1.]];
    let points: Vec<Point<World>> = corners.iter().map(|&[x, y]| point(x, y, 0.0)).collect();
    let ring = |k: usize| [(k, k + 1), (k + 1, k + 2), (k + 2, k + 3), (k + 3, k)];
    let edges = ring(0).into_iter().chain(ring(4)).map(|(a, b)| segment(points[a], points[b], (a, b))).collect();
    let centre = Vec3::new(1.5, 1.5, 0.0);
    let plane = SurfaceRecord(Arc::new(Plane::placed(&facing(Vec3::new(0.0, 0.0, 1.0), centre))));
    let outer = (vec![(0, false), (1, false), (2, false), (3, false)], UvPoint::new(-1.0, -1.0));
    let hole = (vec![(4, false), (5, false), (6, false), (7, false)], UvPoint::new(0.0, 0.0));
    let faces = vec![FaceSpec { surface: 0, reversed: false, loops: vec![outer, hole] }];
    Spec { kind: BodyKind::Sheet, points, edges, surfaces: vec![plane], faces, wires: Vec::new() }
}

/// [`sheet`] with a second loop of the same edges run the other way, its own reversal attached as a hole (S1's
/// 52-case bug): every point of the hole is inside the outer loop, the outer loop's witness included.
pub(crate) fn sheet_holed_by_its_reversal() -> Spec {
    let mut spec = sheet();
    let face = &mut spec.faces[0];
    let (outer, witness) = face.loops[0].clone();
    let reversal = outer.iter().rev().map(|&(edge, reversed)| (edge, !reversed)).collect();
    face.loops.push((reversal, witness));
    spec
}
