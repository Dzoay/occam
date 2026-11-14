//! Source: original
//! Writing wires and faces into an edit: vertices shared by identity and merged by the band as they are made (S1 item
//! 1), each edge on its curve with its two ends in their vertices' stars, a wire body for a wire, and a sheet of one face
//! for a face, each use of its edges with its image in the plane's parameters.

use core::f64::consts::TAU;
use std::sync::Arc;

use occam_geom::{Circle, Line, Plane};
use occam_report::{Code, Fault, Input, NamedValue, Quantity, Remedy, RemedyKind, Report, StepRole, Value};
use occam_topo::{
    Around, Body, BodyKind, BodyRecord, CurveRecord, Edge, EdgeRecord, EdgeRole, EdgeUse, EdgeUseRecord, Edit, End,
    Face, FaceRecord, Id, Loop, LoopRecord, Sheet, Shell, ShellRecord, SurfaceRecord, UseOwner, Vertex, VertexRecord,
    Wire, WireRecord,
};
use occam_units::{Direction, Length, Local, Point, UvPoint, Vec3};

use super::given::Piece;
use super::space::{Bent, Flatland, NoCircle, frame, through, unit};

/// An edge to make: its curve, the part of it the edge covers, and where it begins and ends.
pub(crate) struct Made {
    curve: CurveRecord,
    range: (f64, f64),
    /// Where it begins.
    pub start: Vec3,
    /// Where it ends.
    pub end: Vec3,
}

/// A point as a report's value.
pub(crate) fn shown(name: &str, p: Vec3) -> NamedValue {
    NamedValue::point(name, [p.x, p.y, p.z])
}

/// A report of `code` about the input at `at`, with `band` as the tolerance in force.
pub(crate) fn refused(code: Code, at: &Input, band: f64) -> Report {
    Report::new(code, Fault::InvalidInput)
        .with_input(at.clone())
        .with_value(NamedValue::length("band", band))
        .with_tolerance_in_force(band)
}

/// The edge `piece` makes, at `at` in the request.
///
/// # Errors
///
/// [`Code::NoLength`] for a line or circle no longer than `band`, and for an arc, what [`no_circle`] reports where its
/// points fix no circle.
pub(crate) fn made(piece: Piece, at: &Input, band: f64) -> Result<Made, Report> {
    let short = |length: f64| refused(Code::NoLength, at, band).with_value(NamedValue::length("length", length));
    match piece {
        Piece::Line { from, to } => {
            let length = (to - from).norm();
            if length <= band {
                return Err(short(length));
            }
            let curve = CurveRecord(Arc::new(Line::new(Point::from_vec3(from), Direction::normalised(to - from))));
            Ok(Made { curve, range: (0.0, length), start: from, end: to })
        }
        Piece::Arc { from, through: middle, to } => {
            let arc = through(from, middle, to, band).map_err(|no| no_circle(no, (from, middle), at, band))?;
            let placement = frame(arc.centre, arc.normal, Some(unit(from - arc.centre)));
            let circle = Circle::placed(&placement, Length::from_millimetres(arc.radius))?;
            Ok(Made { curve: CurveRecord(Arc::new(circle)), range: (0.0, arc.sweep), start: from, end: to })
        }
        Piece::Circle { centre, radius, normal } => {
            if radius <= band {
                return Err(short(radius));
            }
            let placement = frame(centre, unit(normal), None);
            let start = placement.point(Point::<Local>::from_vec3(Vec3::new(radius, 0.0, 0.0))).to_vec3();
            let circle = Circle::placed(&placement, Length::from_millimetres(radius))?;
            Ok(Made { curve: CurveRecord(Arc::new(circle)), range: (0.0, TAU), start, end: start })
        }
    }
}

/// The report that the arc at `at` from `from` through `middle` fixes no circle, for the reason `no`.
///
/// [`Code::PointsInLine`] where its points are in line within `band`, with how far the middle one is from the line
/// through its ends. [`Code::ArcEndsMeet`] where its ends are within `band` of each other, with how far apart they are,
/// and the whole circle to write instead, if its radius is longer than the band: the one with `from` and `middle` at the
/// ends of a diameter, square to the world's axis furthest from lying along that diameter.
fn no_circle(no: NoCircle, (from, middle): (Vec3, Vec3), at: &Input, band: f64) -> Report {
    match no {
        NoCircle::InLine(off) => refused(Code::PointsInLine, at, band).with_value(NamedValue::length("offset", off)),
        NoCircle::EndsMeet(gap) => {
            let report = refused(Code::ArcEndsMeet, at, band).with_value(NamedValue::length("gap", gap));
            let radius = (middle - from).norm() * 0.5;
            if radius <= band {
                return report;
            }
            let normal = square_to(unit(middle - from));
            let remedy = Remedy::new(RemedyKind::UseCircle)
                .with_value(shown("centre", (from + middle) * 0.5))
                .with_value(NamedValue::length("radius", radius))
                .with_value(NamedValue { name: "normal".to_string(), quantity: Quantity::Ratio, value: normal });
            report.with_remedy(remedy)
        }
    }
}

/// A unit vector square to the unit vector `along`: the world's axis furthest from lying along it, the first of x, y and
/// z where two are as far, less its part along it, as coordinates.
fn square_to(along: Vec3) -> Value {
    let lean = |axis: Vec3| axis.dot(along).abs();
    let axes = [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    let furthest =
        axes.into_iter().fold(Vec3::new(1.0, 0.0, 0.0), |best, axis| if lean(axis) < lean(best) { axis } else { best });
    let square = unit(furthest - along * furthest.dot(along));
    Value::Point([square.x, square.y, square.z])
}

/// The edges of a wire, made from `pieces` in order, each with where it is in the request, and whether the wire closes.
pub(crate) struct Chain {
    /// The edges, in order.
    pub edges: Vec<Id<Edge>>,
    /// Whether the last ends where the first begins.
    pub closed: bool,
}

/// Where each vertex of a chain of `made` is: each edge's start, then the last's end unless it meets the first's start.
///
/// # Errors
///
/// [`Code::WireGap`] where an edge begins further than `band` from where the one before it ends, naming the edge.
fn corners(made: &[(Made, Input)], band: f64) -> Result<(Vec<Vec3>, bool), Report> {
    let mut points: Vec<Vec3> = made.first().map(|(m, _)| m.start).into_iter().collect();
    for pair in made.windows(2) {
        let [(before, _), (after, at)] = pair else { continue };
        let gap = (after.start - before.end).norm();
        if gap > band {
            let remedy = Remedy::new(RemedyKind::MeetAt).with_value(shown("point", before.end));
            return Err(refused(Code::WireGap, at, band)
                .with_value(NamedValue::length("gap", gap))
                .with_value(shown("end", before.end))
                .with_remedy(remedy));
        }
        points.push(before.end);
    }
    let (first, last) = (made.first().map(|(m, _)| m.start), made.last().map(|(m, _)| m.end));
    let closed = matches!((first, last), (Some(a), Some(b)) if (b - a).norm() <= band);
    if !closed {
        points.extend(last);
    }
    Ok((points, closed))
}

/// The next of `n` places round each ring of `rings`, in the order each ring lists them.
fn successors(rings: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut next: Vec<usize> = (0..n).collect();
    for ring in rings {
        let after = ring.iter().skip(1).chain(ring.first());
        for (&place, &following) in ring.iter().zip(after) {
            if let Some(slot) = next.get_mut(place) {
                *slot = following;
            }
        }
    }
    next
}

/// Makes the edges of `made`, in order, with their vertices, in `edit`.
///
/// # Errors
///
/// [`Code::WireGap`] where two edges do not meet within `band`.
pub(crate) fn chain(edit: &mut Edit, made: Vec<(Made, Input)>, band: f64) -> Result<Chain, Report> {
    let (points, closed) = corners(&made, band)?;
    let vertices: Vec<Id<Vertex>> = points.iter().map(|_| edit.reserve()).collect();
    let edges: Vec<Id<Edge>> = made.iter().map(|_| edit.reserve()).collect();
    let corners_made = vertices.len();
    let ends = |k: usize| {
        let after = k.saturating_add(1);
        (k, if closed { after.checked_rem(corners_made).unwrap_or(0) } else { after })
    };
    // Place 2k is edge k's start and 2k + 1 its finish; each vertex's star lists its places in the order the edges do.
    let mut rings: Vec<Vec<usize>> = vec![Vec::new(); corners_made];
    for k in 0..edges.len() {
        let (begins, finishes) = ends(k);
        rings.get_mut(begins).into_iter().for_each(|ring| ring.push(k.saturating_mul(2)));
        rings.get_mut(finishes).into_iter().for_each(|ring| ring.push(k.saturating_mul(2).saturating_add(1)));
    }
    let round = successors(&rings, edges.len().saturating_mul(2));
    let place = |p: usize| -> Option<Around> {
        let end = if p.is_multiple_of(2) { End::Start } else { End::Finish };
        edges.get(p.checked_div(2)?).map(|&edge| Around { edge, end })
    };
    let born = edit.provenance(StepRole::Made);
    for (k, (piece, _)) in made.into_iter().enumerate() {
        let (begins, finishes) = ends(k);
        let after = |end: usize| round.get(k.saturating_mul(2).saturating_add(end)).copied().and_then(place);
        let (Some(round_start), Some(round_finish), Some(&first), Some(&last), Some(&edge)) =
            (after(0), after(1), vertices.get(begins), vertices.get(finishes), edges.get(k))
        else {
            continue;
        };
        let curve = edit.add(piece.curve);
        let record = EdgeRecord {
            curve,
            range: piece.range,
            ends: (first, last),
            around: (round_start, round_finish),
            first_use: None,
            role: EdgeRole::Ordinary,
            deviation: Length::from_millimetres(0.0),
            born,
        };
        edit.set(edge, record)?;
    }
    for ((&vertex, point), ring) in vertices.iter().zip(points).zip(&rings) {
        let star = ring.first().copied().and_then(place);
        let record = VertexRecord { point: Point::from_vec3(point), radius: Length::from_millimetres(0.0), star };
        edit.set(vertex, record)?;
    }
    Ok(Chain { edges, closed })
}

/// Gives `edge` the use `first` as the first of its uses.
fn first_use(edit: &mut Edit, edge: Id<Edge>, first: Id<EdgeUse>) -> Result<(), Report> {
    let Some(record) = edit.record(edge) else { return Ok(()) };
    edit.set(edge, EdgeRecord { first_use: Some(first), ..record })
}

/// The uses of `edges` in order by `owner`, each the only use of its edge, round a ring if `closed`.
///
/// # Errors
///
/// None: every id is one this edit reserved.
fn uses(
    edit: &mut Edit,
    edges: &[(Id<Edge>, bool)],
    owner: UseOwner,
    closed: bool,
) -> Result<Vec<Id<EdgeUse>>, Report> {
    let ids: Vec<Id<EdgeUse>> = edges.iter().map(|_| edit.reserve()).collect();
    let after: Vec<Option<Id<EdgeUse>>> =
        ids.iter().skip(1).map(|&u| Some(u)).chain([closed.then(|| ids.first().copied()).flatten()]).collect();
    for ((&id, &(edge, reversed)), next) in ids.iter().zip(edges).zip(after) {
        edit.set(id, EdgeUseRecord { edge, owner, next, radial: id, reversed, pcurve: None })?;
        first_use(edit, edge, id)?;
    }
    Ok(ids)
}

/// Makes a wire body of `chain`, and gives the body.
///
/// # Errors
///
/// None: every id is one this edit reserved.
pub(crate) fn wire_body(edit: &mut Edit, chain: &Chain) -> Result<Id<Body>, Report> {
    let (body, wire) = (edit.reserve::<Body>(), edit.reserve::<Wire>());
    let edges: Vec<(Id<Edge>, bool)> = chain.edges.iter().map(|&e| (e, false)).collect();
    let used = uses(edit, &edges, UseOwner::Wire(wire), chain.closed)?;
    if let Some(&first) = used.first() {
        edit.set(wire, WireRecord { body, first, next: None })?;
    }
    let born = edit.provenance(StepRole::Made);
    edit.set(body, BodyRecord { kind: BodyKind::Wire, first_shell: None, first_wire: Some(wire), born })?;
    Ok(body)
}

/// A loop of a face to make: its edges in order, each with whether its use runs against it, and its witness.
pub(crate) struct Ring {
    /// The edges, in the order the loop runs.
    pub edges: Vec<(Id<Edge>, bool)>,
    /// A point strictly inside it, in the plane's parameters.
    pub witness: UvPoint,
}

/// Makes a sheet of one face on `plane`, bounded by `rings`, the outer first, and gives the face.
///
/// # Errors
///
/// What a parameter curve's family reports, which a line or circle in the plane does not.
pub(crate) fn sheet(
    edit: &mut Edit,
    plane: &Flatland,
    rings: &[Ring],
    bents: &[Vec<Bent>],
) -> Result<Id<Face>, Report> {
    let surface = edit.add(SurfaceRecord(Arc::new(Plane::placed(&plane.placement))));
    let (body, shell, face) = (edit.reserve::<Body>(), edit.reserve::<Shell>(), edit.reserve::<Face>());
    let loops: Vec<Id<Loop>> = rings.iter().map(|_| edit.reserve()).collect();
    for (k, (ring, shapes)) in rings.iter().zip(bents).enumerate() {
        let (Some(&own), next) = (loops.get(k), loops.get(k.saturating_add(1)).copied()) else { continue };
        let used = uses(edit, &ring.edges, UseOwner::Loop(own), true)?;
        for (&u, &bent) in used.iter().zip(shapes) {
            let pcurve = edit.add(plane.pcurve(bent)?);
            if let Some(record) = edit.record(u) {
                edit.set(u, EdgeUseRecord { pcurve: Some(pcurve), ..record })?;
            }
        }
        let Some(&first) = used.first() else { continue };
        edit.set(own, LoopRecord { face, first, next, witness: ring.witness })?;
    }
    let born = edit.provenance(StepRole::Made);
    let outer = loops.first().copied().unwrap_or_else(|| edit.reserve());
    edit.set(face, FaceRecord { shell, surface, sheet: Sheet::default(), reversed: false, outer, born })?;
    edit.set(shell, ShellRecord { body, face, next: None })?;
    edit.set(body, BodyRecord { kind: BodyKind::Sheet, first_shell: Some(shell), first_wire: None, born })?;
    Ok(face)
}
