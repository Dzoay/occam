//! Source: original
//! Whether a solid faces out (PARITY 1.10, orientation). Each loop winding the way its face faces, and each edge's two
//! uses running opposite ways, make every face of a shell face the way its neighbours do; whether that is out of the
//! solid or into it only the whole body says. It says so where it reaches furthest along an axis, at a point inside a
//! face, on an edge or at a vertex. The solid lies wholly on one side of the plane through that point across the axis,
//! so over a small ball about the point the axis's flux out through the faces is minus its flux out through the part of
//! the ball's sphere inside the solid, all of which lies below the plane (the divergence theorem, for a field with
//! none): it is positive. So the outward normals of the faces meeting there, each weighted by the angle its face spans
//! about the point in its tangent plane (a whole turn inside a face, half a turn along an edge), point along the axis on
//! the whole if the solid faces out, and against it if it faces in.
//!
//! The furthest point is found on the edges' curves, at an end or, where a curve's box reaches past both ends, between
//! them, from even samples narrowed by golden section; and inside a face on a sphere or a torus, at the point its
//! surface reaches furthest, where the face holds it. A plane, a cylinder and a cone reach no further than their edges,
//! and a face on any other surface must keep within its surface's box over the box its outer loop's parameter curves
//! span, or over the surface's domain where a use has none. Where the furthest point is not certain, or the faces there
//! point neither clearly along the axis nor clearly against it, the next axis is tried, and a solid that no axis decides
//! passes: the check refuses only what it shows. With several shells, the one that reaches furthest is the outer one,
//! and only it is asked.

use core::f64::consts::{PI, TAU};
use std::collections::BTreeSet;

use occam_geom::{Analytic, Curve, Scratch, Surface, UvBox};
use occam_math::atan2;
use occam_report::{EntityRef, NamedValue, Quantity, Reason, Report};
use occam_units::{Bounds, Point, UvPoint, Vec3};

use super::failed;
use super::trace::Trace;
use crate::check::read::note;
use crate::kind::{Body, Edge, EdgeUse, Face, Id, Kind, Loop, Shell, Vertex};
use crate::records::{End, UseOwner};
use crate::snapshot::Model;
use crate::view::{View, view};

/// Each axis, either way, in the order they are tried.
const AXES: [Vec3; 6] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, -1.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(0.0, 0.0, -1.0),
];

/// How clearly the faces at the furthest point must point along the axis, or against it, to decide: the sum of their
/// normals' parts along it, each weighted by its angle there, is at least this far from zero. A tenth of a radian is a
/// sixteenth of what a box's corner gives, and keeps a point that is furthest only to within the slack from deciding.
const CLEAR: f64 = 0.1;

/// How near to no angle, or to a whole turn, a face's corner is a cusp, whose side cannot be told.
const CUSP: f64 = 1e-6;

/// How many bands one place may fall short of another and still be as far: ends of edges that meet at a vertex may lie
/// a band either side of it.
const SLACK: f64 = 4.0;

/// How many even steps an edge is sampled at to find where it reaches furthest.
const SAMPLES: u32 = 16;

/// How many times the step either side of the furthest sample is narrowed, each time to the golden ratio of itself.
const NARROWINGS: u32 = 64;

/// The golden ratio's reciprocal, `(sqrt 5 - 1) / 2`.
const GOLDEN: f64 = 0.618_033_988_749_894_8;

/// Where a body reaches furthest along an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    /// At a vertex, at an end of an edge's curve.
    Vertex(Id<Vertex>),
    /// On an edge's curve between its ends, at this parameter.
    Edge(Id<Edge>, f64),
    /// Inside a face, at this point of its surface's parameters.
    Face(Id<Face>, UvPoint),
}

/// A place, its point, and how far along the axis that is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Furthest {
    place: Place,
    point: Vec3,
    reach: f64,
}

/// What a body is made of, read once: its faces, the uses of their loops each with its face, and their edges in id
/// order.
struct Parts<'a> {
    faces: Vec<View<'a, Face>>,
    uses: Vec<(View<'a, EdgeUse>, View<'a, Face>)>,
    edges: Vec<View<'a, Edge>>,
}

impl<'a> Parts<'a> {
    /// The parts of `body`, if every record they read is there.
    fn of(model: &'a Model, body: Id<Body>) -> Option<Self> {
        let shells = read(model, body)?.shells();
        let faces: Option<Vec<View<'a, Face>>> = shells
            .into_iter()
            .filter_map(|s| read(model, s))
            .flat_map(View::<Shell>::faces)
            .map(|f| read(model, f))
            .collect();
        let faces = faces?;
        let mut uses = Vec::new();
        for &f in &faces {
            for l in f.loops() {
                for u in read::<Loop>(model, l)?.uses() {
                    uses.push((read(model, u)?, f));
                }
            }
        }
        let ids: BTreeSet<Id<Edge>> = uses.iter().map(|(u, _)| u.edge()).collect();
        let edges: Option<Vec<View<'a, Edge>>> = ids.into_iter().map(|e| read(model, e)).collect();
        Some(Self { faces, uses, edges: edges? })
    }
}

/// A view of `id`, counted as the checks' reads are.
fn read<K: Kind>(model: &Model, id: Id<K>) -> Option<View<'_, K>> {
    note();
    view(model, id)
}

/// The report of solid `body` if its faces point into the volume it bounds, as the first axis that decides shows:
/// naming the shell that reaches furthest and the vertex, edge or face where it does, with that point and how far the
/// faces there point along the axis, below zero.
pub(crate) fn inside_out(model: &Model, body: Id<Body>) -> Option<Report> {
    let parts = Parts::of(model, body)?;
    let slack = slack(model);
    for axis in AXES {
        let Some((at, facing)) = decide(model, &parts, axis, slack) else { continue };
        if facing.is_sign_positive() {
            return None;
        }
        let named = [shell_at(model, at.place)?, entity(at.place)];
        let [x, y, z] = [at.point.x, at.point.y, at.point.z];
        let report = failed(Reason::ShellInsideOut, &named).with_value(NamedValue::point("furthest", [x, y, z]));
        return Some(report.with_value(NamedValue::new("facing", Quantity::Angle, facing)));
    }
    None
}

/// Where `parts` reach furthest along `axis`, and how far the faces there point along it, if both are certain and the
/// faces point clearly one way.
fn decide(model: &Model, parts: &Parts<'_>, axis: Vec3, slack: f64) -> Option<(Furthest, f64)> {
    let at = furthest(model, parts, axis, slack)?;
    let facing = facing(model, parts, at, axis, slack)?;
    clear(facing).then_some((at, facing))
}

/// Whether faces that point `facing` along an axis point clearly along it, or against it: at least [`CLEAR`] either way.
fn clear(facing: f64) -> bool {
    facing.abs() >= CLEAR
}

/// How far one place may fall short of another along an axis and still count as far: [`SLACK`] bands.
fn slack(model: &Model) -> f64 {
    SLACK * model.tolerance.band().millimetres()
}

/// The entity at `place`.
const fn entity(place: Place) -> EntityRef {
    match place {
        Place::Vertex(v) => v.entity(),
        Place::Edge(e, _) => e.entity(),
        Place::Face(f, _) => f.entity(),
    }
}

/// The shell of a face at `place`: the face, or that of an edge's first use there.
fn shell_at(model: &Model, place: Place) -> Option<EntityRef> {
    let edge = match place {
        Place::Vertex(v) => *view(model, v)?.edges().first()?,
        Place::Edge(e, _) => e,
        Place::Face(f, _) => return Some(view(model, f)?.shell().entity()),
    };
    let first = *view(model, edge)?.uses().first()?;
    let UseOwner::Loop(l) = view(model, first)?.owner() else { return None };
    Some(view(model, view(model, l)?.face())?.shell().entity())
}

/// Whether `reach` is past `best` by more than `slack`.
fn past(reach: f64, best: f64, slack: f64) -> bool {
    reach > best + slack
}

/// The larger of two numbers, the first if they are equal.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The smaller of two numbers, the first if they are equal.
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The further of two places, the first if they are as far.
fn further(a: Option<Furthest>, b: Option<Furthest>) -> Option<Furthest> {
    match (a, b) {
        (Some(x), Some(y)) if y.reach > x.reach => Some(y),
        (None, found) => found,
        (kept, _) => kept,
    }
}

/// How far along `axis` the box `bounds` reaches.
fn along(bounds: Bounds, axis: Vec3) -> f64 {
    let (low, high) = (bounds.low().to_vec3(), bounds.high().to_vec3());
    let part = |a: f64, lo: f64, hi: f64| {
        if a > 0.0 {
            a * hi
        } else if a < 0.0 {
            a * lo
        } else {
            0.0
        }
    };
    part(axis.x, low.x, high.x) + part(axis.y, low.y, high.y) + part(axis.z, low.z, high.z)
}

/// Where `parts` reach furthest along `axis`, if that is certain: no edge's curve's box, and no face's where its
/// furthest point is not known, reaches past it by more than `slack`.
fn furthest(model: &Model, parts: &Parts<'_>, axis: Vec3, slack: f64) -> Option<Furthest> {
    let (mut best, mut bound) = (None, f64::NEG_INFINITY);
    for &e in &parts.edges {
        let curve = e.curve()?;
        let reach = along(curve.bounds(e.range().0, e.range().1), axis);
        bound = larger(bound, reach);
        best = further(best, Some(on_edge(e, curve, axis, reach, slack)));
    }
    for &f in &parts.faces {
        let beyond = best.map_or(f64::NEG_INFINITY, |b: Furthest| b.reach);
        match inside(model, f, axis, (beyond, slack)) {
            Inside::Nothing => {}
            Inside::Point(found) => best = further(best, Some(found)),
            Inside::Boxed(reach) => bound = larger(bound, reach),
        }
    }
    best.filter(|b| !past(bound, b.reach, slack))
}

/// Where edge `e`, on `curve`, reaches furthest along `axis`: at an end, unless the curve's box reaches `bound`, past
/// both ends by more than `slack`, and then where the curve does between them, if that is further.
fn on_edge(e: View<'_, Edge>, curve: &dyn Curve, axis: Vec3, bound: f64, slack: f64) -> Furthest {
    let ((lo, hi), (start, finish)) = (e.range(), e.ends());
    let at = |t: f64| curve.point(t).to_vec3();
    let (a, b) = (at(lo), at(hi));
    let end = if b.dot(axis) > a.dot(axis) {
        Furthest { place: Place::Vertex(finish), point: b, reach: b.dot(axis) }
    } else {
        Furthest { place: Place::Vertex(start), point: a, reach: a.dot(axis) }
    };
    if !past(bound, end.reach, slack) {
        return end;
    }
    let (t, point) = peak(curve, (lo, hi), axis);
    let between = Furthest { place: Place::Edge(e.id(), t), point, reach: point.dot(axis) };
    if between.reach > end.reach { between } else { end }
}

/// The parameter in `(lo, hi)` where `curve` reaches furthest along `axis`, and its point there: the furthest of even
/// samples, or of the step either side of it narrowed, if that is further.
fn peak(curve: &dyn Curve, (lo, hi): (f64, f64), axis: Vec3) -> (f64, Vec3) {
    let reach = |t: f64| curve.point(t).to_vec3().dot(axis);
    let sample = |k: u32| if k >= SAMPLES { hi } else { lo + (hi - lo) * (f64::from(k) / f64::from(SAMPLES)) };
    let best = (0..=SAMPLES).fold(0, |best, k| if reach(sample(k)) > reach(sample(best)) { k } else { best });
    let narrowed = narrow(&reach, (sample(best.saturating_sub(1)), sample(best.saturating_add(1))));
    let t = if reach(narrowed) > reach(sample(best)) { narrowed } else { sample(best) };
    (t, curve.point(t).to_vec3())
}

/// Where in `(from, to)` `reach` is greatest, narrowed by golden section [`NARROWINGS`] times: each time, of two points
/// dividing the step in the golden ratio, the one nearer the lower keeps the step's end beyond it.
fn narrow(reach: &impl Fn(f64) -> f64, (mut from, mut to): (f64, f64)) -> f64 {
    let (mut left, mut right) = (to - GOLDEN * (to - from), from + GOLDEN * (to - from));
    let (mut at_left, mut at_right) = (reach(left), reach(right));
    for _ in 0..NARROWINGS {
        if at_left > at_right {
            (to, right, at_right) = (right, left, at_left);
            left = to - GOLDEN * (to - from);
            at_left = reach(left);
        } else {
            (from, left, at_left) = (left, right, at_right);
            right = from + GOLDEN * (to - from);
            at_right = reach(right);
        }
    }
    if at_left > at_right { left } else { right }
}

/// What inside a face may reach further along an axis than its edges.
enum Inside {
    /// Nothing.
    Nothing,
    /// This place, inside it.
    Point(Furthest),
    /// Anything up to this far.
    Boxed(f64),
}

/// What inside face `f` may reach further along `axis` than its edges, and past `beyond` by more than `slack`.
fn inside(model: &Model, f: View<'_, Face>, axis: Vec3, (beyond, slack): (f64, f64)) -> Inside {
    let Some(surface) = f.surface() else { return Inside::Boxed(f64::INFINITY) };
    let turning = match surface.analytic(model.tolerance) {
        Some(Analytic::Plane { .. } | Analytic::Cylinder { .. } | Analytic::Cone { .. }) => return Inside::Nothing,
        Some(Analytic::Sphere { centre, radius }) => centre.to_vec3() + axis * radius.millimetres(),
        Some(Analytic::Torus { centre, axis: own, major, minor }) => {
            // Along its own axis it reaches as far round whole circles, which cross its faces' edges.
            let flat = axis - own.get() * axis.dot(own.get());
            if flat.norm() <= 1e-12 {
                return Inside::Nothing;
            }
            centre.to_vec3() + flat * (major.millimetres() / flat.norm()) + axis * minor.millimetres()
        }
        None => {
            let domain = surface.domain();
            let whole = UvBox::new(UvPoint::new(domain.u.lo, domain.v.lo), UvPoint::new(domain.u.hi, domain.v.hi));
            return Inside::Boxed(along(surface.bounds(region(model, f).unwrap_or(whole)), axis));
        }
    };
    if !past(turning.dot(axis), beyond, slack) {
        return Inside::Nothing;
    }
    holds(model, f, surface, turning, axis)
}

/// The box in face `f`'s parameters that its outer loop's parameter curves span, if each of its uses has one: the face
/// lies in it, as it lies inside its outer loop.
fn region(model: &Model, f: View<'_, Face>) -> Option<UvBox> {
    let spans: Option<Vec<UvBox>> = view(model, f.outer())?
        .uses()
        .into_iter()
        .map(|u| {
            let (u, (lo, hi)) = view(model, u).and_then(|u| Some((u, view(model, u.edge())?.range())))?;
            Some(u.pcurve()?.bounds(lo, hi))
        })
        .collect();
    let corner =
        |a: UvPoint, b: UvPoint, pick: fn(f64, f64) -> f64| UvPoint::new(pick(a.u(), b.u()), pick(a.v(), b.v()));
    spans?.into_iter().reduce(|a, b| UvBox::new(corner(a.lo, b.lo, smaller), corner(a.hi, b.hi, larger)))
}

/// Whether face `f`, on `surface`, holds the point `p`, which reaches `p . axis` along `axis`: strictly inside its outer
/// loop and outside its holes, or strictly outside it. Where that cannot be told, the face may reach as far as `p`.
fn holds(model: &Model, f: View<'_, Face>, surface: &dyn Surface, p: Vec3, axis: Vec3) -> Inside {
    let (band, reach) = (model.tolerance.band().millimetres(), p.dot(axis));
    let found = surface.invert(Point::from_vec3(p), None, model.tolerance, &mut Scratch::default());
    let traces: Option<Vec<Trace<'_>>> =
        f.loops().into_iter().map(|l| view(model, l).and_then(|l| Trace::of(model, l).ok())).collect();
    let (Ok(uv), Some(traces)) = (found, traces) else { return Inside::Boxed(reach) };
    let Some((outer, holes)) = traces.split_first() else { return Inside::Boxed(reach) };
    let q = outer.placed(uv);
    let within = outer.place(q, band);
    if within.outside(band) {
        return Inside::Nothing;
    }
    let places: Vec<_> = holes.iter().map(|hole| hole.place(q, band)).collect();
    if places.iter().any(|place| place.inside(band)) {
        Inside::Nothing
    } else if within.inside(band) && places.iter().all(|place| place.outside(band)) {
        Inside::Point(Furthest { place: Place::Face(f.id(), q), point: p, reach })
    } else {
        Inside::Boxed(reach)
    }
}

/// How far the faces at `at` point along `axis`: their outward normals' parts along it there, each weighted by the angle
/// its face spans about the point, summed; `None` where a normal or an angle cannot be had, or a curve's end at a
/// vertex is further than `slack` from the point.
fn facing(model: &Model, parts: &Parts<'_>, at: Furthest, axis: Vec3, slack: f64) -> Option<f64> {
    match at.place {
        Place::Face(f, uv) => Some(TAU * outward(view(model, f)?, uv)?.dot(axis)),
        Place::Edge(e, t) => {
            let sides = parts.uses.iter().filter(|(u, _)| u.edge() == e);
            sides.map(|&(u, f)| Some(PI * outward(f, on(model, u, f, t)?)?.dot(axis))).sum()
        }
        Place::Vertex(v) => {
            let ending = parts.uses.iter().filter(|(u, _)| u.vertices().map(|(_, to)| to) == Some(v));
            ending.map(|&(u, f)| corner(model, (u, f), at.point, slack).map(|(angle, n)| angle * n.dot(axis))).sum()
        }
    }
}

/// The outward unit normal of face `f` at `uv`.
fn outward(f: View<'_, Face>, uv: UvPoint) -> Option<Vec3> {
    Some(normal(f.surface()?, f.reversed(), uv))
}

/// The unit normal of `surface` at `uv`, turned over if `reversed`. Where the surface collapses to a point, as at an
/// apex, it is not a number, and so decides nothing: every comparison with it fails.
fn normal(surface: &dyn Surface, reversed: bool, uv: UvPoint) -> Vec3 {
    let jet = surface.first(uv);
    let across = jet.du.rate().cross(jet.dv.rate());
    let sign = if reversed { -1.0 } else { 1.0 };
    across * (sign / across.norm())
}

/// The point of face `f`'s parameters where use `u` is at parameter `t` of its edge.
fn on(model: &Model, u: View<'_, EdgeUse>, f: View<'_, Face>, t: f64) -> Option<UvPoint> {
    if let Some(pcurve) = u.pcurve() {
        return Some(pcurve.uv(t));
    }
    let curve = view(model, u.edge())?.curve()?;
    f.surface()?.invert(curve.point(t), None, model.tolerance, &mut Scratch::default()).ok()
}

/// Where use `u` is at its `end`: its edge's parameter, its point, and the way it runs there, as fast as its curve.
fn travel(model: &Model, u: View<'_, EdgeUse>, end: End) -> Option<(f64, Vec3, Vec3)> {
    let e = view(model, u.edge())?;
    let (lo, hi) = e.range();
    let t = if (end == End::Start) == u.reversed() { hi } else { lo };
    let (point, tangent) = e.curve()?.first(t);
    let along = if u.reversed() { -tangent.rate() } else { tangent.rate() };
    Some((t, point.to_vec3(), along))
}

/// The corner of face `f` where use `into` ends and the next use begins, at `point`: the angle it spans there, from
/// the way out round anticlockwise about the face's outward normal to the way in, backwards; and that normal. `None`
/// where either use's curve ends further than `slack` from `point`, or at a cusp.
fn corner(
    model: &Model,
    (into, f): (View<'_, EdgeUse>, View<'_, Face>),
    point: Vec3,
    slack: f64,
) -> Option<(f64, Vec3)> {
    let out = view(model, into.next()?)?;
    let (_, arrive, arriving) = travel(model, into, End::Finish)?;
    let (start, leave, leaving) = travel(model, out, End::Start)?;
    if (arrive - point).norm() > slack || (leave - point).norm() > slack {
        return None;
    }
    // Both ways lie in the face's tangent plane, as a curve on its surface runs, and the angle between them does not
    // depend on how fast either runs.
    let normal = outward(f, on(model, out, f, start)?)?;
    let back = -arriving;
    Some((spanned(atan2(normal.dot(leaving.cross(back)), leaving.dot(back)))?, normal))
}

/// The angle a corner spans from the angle `turn`, from -π to π, it turns from the way out to the way in, backwards:
/// anticlockwise, from just above none to a whole turn, `None` at a cusp, within [`CUSP`] of either.
fn spanned(turn: f64) -> Option<f64> {
    let angle = if turn > 0.0 { turn } else { turn + TAU };
    (angle > CUSP && angle < TAU - CUSP).then_some(angle)
}

#[cfg(test)]
mod tests;
