//! Source: original
//! The checks that read geometry (S1), one entity at a time. Each check belongs to one entity, so a check of the whole
//! model reports each failure once: a vertex checks the ends of the edges round its star, an edge its range and the
//! parameter curves of its uses, a loop where it lies in its face's parameters, and a face how its loops nest. A check
//! reads only links that agree with each other; where they do not, the check of the links reports it.
//!
//! A use's parameter curve runs over its edge's range, in the edge's direction. It agrees with the edge within the band
//! widened by the edge's measured deviation: an edge may stray from its faces by more than the band (S1 item 9: up to
//! 140 times it), and a curve that follows the face's surface is then no closer to it. `commit` measures that
//! deviation, at even steps along each edge it created or changed or whose uses it did, and stores it, so no caller
//! asserts it.

#[cfg(test)]
mod reach;
#[cfg(test)]
mod refused;
mod trace;
mod winding;

use core::num::NonZeroU32;
use std::collections::BTreeSet;

use occam_geom::{Curve, Scratch, join_edge};
use occam_report::{Code, EntityKind, EntityRef, Fault, NamedValue, Quantity, Reason, Report};
use occam_units::{Length, Tolerance};

use crate::edit::Edit;
use crate::kind::sealed::Sealed;
use crate::kind::{Edge, EdgeUse, Face, Id, Kind, Loop, Vertex};
use crate::records::{Around, End, UseOwner};
use crate::snapshot::Model;
use crate::view::{View, most, view, walk};
use trace::{Off, Trace, Untraced};

/// How many even steps an edge's deviation is measured at, its ends included: as many as a parameter curve is
/// joined to its edge at.
const STEPS: u32 = 32;

/// The report of one failure.
fn failed(reason: Reason, entities: &[EntityRef]) -> Report {
    let report = Report::new(Code::InvariantFailed, Fault::Internal).with_reason(reason);
    entities.iter().fold(report, |report, &entity| report.with_entity(entity))
}

/// The id `entity` names, if it is of kind `K`.
fn id<K: Kind>(entity: EntityRef) -> Option<Id<K>> {
    (entity.kind == K::ENTITY).then_some(())?;
    Some(Id::new(entity.slot, NonZeroU32::new(entity.generation)?))
}

/// Whether `x` is no more than `limit`; never for NaN.
fn within(x: f64, limit: f64) -> bool {
    x <= limit
}

/// Every geometric invariant of one entity, reading its record, its neighbours' records and their geometry, in a
/// fixed order.
pub(crate) fn entity(model: &Model, entity: EntityRef, out: &mut Vec<Report>) {
    match entity.kind {
        EntityKind::Vertex => {
            if let Some(v) = id::<Vertex>(entity).and_then(|v| view(model, v)) {
                vertex(model, v, out);
            }
        }
        EntityKind::Edge => {
            if let Some(e) = id::<Edge>(entity).and_then(|e| view(model, e)) {
                edge(model, e, out);
            }
        }
        EntityKind::Loop => {
            if let Some(l) = id::<Loop>(entity).and_then(|l| view(model, l)) {
                bounding(model, l, out);
            }
        }
        EntityKind::Face => {
            if let Some(f) = id::<Face>(entity).and_then(|f| view(model, f)) {
                nesting(model, f, out);
            }
        }
        _ => {}
    }
}

/// Each end of an edge round `v`'s star that ends at `v` lies within its certified radius and the band of it.
fn vertex(model: &Model, v: View<'_, Vertex>, out: &mut Vec<Report>) {
    let band = model.tolerance.band().millimetres();
    let radius = v.radius().millimetres();
    let next = |place: Around| view(model, place.edge).map(|e| e.around(place.end));
    for place in walk(v.star(), most::<Edge>(model).saturating_mul(2), next) {
        let Some(edge) = view(model, place.edge) else { continue };
        let ((start, finish), (lo, hi)) = (edge.ends(), edge.range());
        let (at, t) = match place.end {
            End::Start => (start, lo),
            End::Finish => (finish, hi),
        };
        let Some(curve) = edge.curve().filter(|_| at == v.id()) else { continue };
        let gap = (curve.point(t).to_vec3() - v.point().to_vec3()).norm();
        if !within(gap, radius + band) {
            out.push(
                failed(Reason::VertexOffEdge, &[v.id().entity(), place.edge.entity()])
                    .with_value(NamedValue::length("distance", gap))
                    .with_value(NamedValue::length("radius", radius))
                    .with_value(NamedValue::new("parameter", curve.range().quantity(), t))
                    .with_tolerance_in_force(band),
            );
        }
    }
}

/// An edge's range lies in its curve's domain, and each of its uses' parameter curves runs along it.
fn edge(model: &Model, e: View<'_, Edge>, out: &mut Vec<Report>) {
    let Some(curve) = e.curve() else { return };
    let ((start, finish), range) = (e.range(), curve.range());
    if range.trimmed(start, finish).is_err() {
        let quantity = range.quantity();
        out.push(
            failed(Reason::OutsideDomain, &[e.id().entity(), e.curve_id().entity()])
                .with_value(NamedValue::new("start", quantity, start))
                .with_value(NamedValue::new("finish", quantity, finish))
                .with_value(NamedValue::new("low", quantity, range.lo))
                .with_value(NamedValue::new("high", quantity, range.hi)),
        );
        return;
    }
    off_face(model, e, curve, out);
    for u in e.uses().into_iter().filter_map(|u| view(model, u)) {
        joined(model, e, curve, u, out);
    }
}

/// The edge's measured deviation is within the band and the near-tangent shift for its length, `sqrt(2 length band)`
/// (S2 item 4): as far as a band decision at near tangency moves a point on a curvature radius that long, and so as far
/// as an honest section strays (S1 item 9 saw 140 bands), while a misplaced curve or surface strays further. The length
/// is the chord's over the steps the deviation is measured at, which is never more than the arc's, so the limit errs
/// strict.
fn off_face(model: &Model, e: View<'_, Edge>, curve: &dyn Curve, out: &mut Vec<Report>) {
    let band = model.tolerance.band();
    let length = chord(curve, e.range());
    let limit = band.millimetres() + model.tolerance.near_tangent_shift(Length::from_millimetres(length)).millimetres();
    let deviation = e.deviation().millimetres();
    if !within(deviation, limit) {
        let entities: Vec<EntityRef> =
            core::iter::once(e.id().entity()).chain(e.faces().into_iter().map(Id::entity)).collect();
        out.push(
            failed(Reason::EdgeOffFace, &entities)
                .with_value(NamedValue::length("distance", deviation))
                .with_value(NamedValue::length("limit", limit))
                .with_value(NamedValue::length("length", length))
                .with_tolerance_in_force(band.millimetres()),
        );
    }
}

/// The parameter `k` steps of [`STEPS`] along `(lo, hi)`, `hi` itself at the end.
fn step((lo, hi): (f64, f64), k: u32) -> f64 {
    if k >= STEPS { hi } else { lo + (hi - lo) * (f64::from(k) / f64::from(STEPS)) }
}

/// The length of the chords of `curve` between its points at the steps of `range`.
fn chord(curve: &dyn Curve, range: (f64, f64)) -> f64 {
    let points: Vec<_> = (0..=STEPS).map(|k| curve.point(step(range, k)).to_vec3()).collect();
    points.windows(2).map(|pair| pair.get(1).zip(pair.first()).map_or(0.0, |(b, a)| (*b - *a).norm())).sum()
}

/// The parameter curve of `u`, if it has one, runs along `e` on its face's surface, the same way, within the band
/// widened by the edge's measured deviation.
fn joined(model: &Model, e: View<'_, Edge>, curve: &dyn Curve, u: View<'_, EdgeUse>, out: &mut Vec<Report>) {
    let (Some(pcurve), UseOwner::Loop(owner)) = (u.pcurve(), u.owner()) else { return };
    let face = view(model, owner).filter(|_| u.edge() == e.id()).and_then(|l| view(model, l.face()));
    let Some((face, surface)) = face.and_then(|f| Some((f, f.surface()?))) else { return };
    let band = model.tolerance.band().millimetres() + e.deviation().millimetres();
    let Ok(tol) = Tolerance::new(Length::from_millimetres(band), model.tolerance.angle()) else { return };
    let Err(found) = join_edge(surface, (pcurve, e.range()), (curve, e.range()), tol) else { return };
    let reason = if found.code == Code::EdgeOffSurface { Reason::ParamCurveOffEdge } else { Reason::OutsideDomain };
    let report = failed(reason, &[e.id().entity(), u.id().entity(), face.id().entity()]);
    let carried = found.values.iter().filter(|value| value.name != "band").cloned();
    out.push(carried.fold(report, Report::with_value).with_tolerance_in_force(band));
}

/// A loop closes in its face's parameters, lies in the period of them its face's sheet names, and holds its witness
/// strictly inside it.
fn bounding(model: &Model, l: View<'_, Loop>, out: &mut Vec<Report>) {
    if !uses_meet(model, l) {
        return;
    }
    let band = model.tolerance.band().millimetres();
    let (this, face) = (l.id().entity(), l.face().entity());
    let trace = match Trace::of(model, l) {
        Ok(trace) => trace,
        Err(Untraced::Links) => return,
        Err(Untraced::Point { of, t, quantity }) => {
            out.push(failed(Reason::LoopNotTraced, &[this, of, face]).with_value(NamedValue::new(
                "parameter",
                quantity,
                t,
            )));
            return;
        }
    };
    let [u, v] = trace.charts.map(|chart| chart.quantity);
    if let Some(joint) = trace.open() {
        out.push(
            failed(Reason::LoopOpenInParameters, &[this, joint.after, joint.before])
                .with_value(NamedValue::new("gap-u", u, joint.gap[0]))
                .with_value(NamedValue::new("gap-v", v, joint.gap[1]))
                .with_tolerance_in_force(band),
        );
        return;
    }
    if let Some(off) = trace.off_sheet() {
        out.push(outside(&off, &[this, face]).with_tolerance_in_force(band));
    }
    let place = trace.place(l.witness(), band);
    if !place.inside(band) {
        let mut report = failed(Reason::WitnessNotInside, &[this, face])
            .with_value(NamedValue::new("u", u, l.witness().u()))
            .with_value(NamedValue::new("v", v, l.witness().v()))
            .with_value(NamedValue::length("distance", place.clearance));
        if let Some(winding) = place.winding {
            report = report.with_value(NamedValue::new("winding", Quantity::Count, f64::from(winding)));
        }
        out.push(report.with_tolerance_in_force(band));
    }
}

/// Whether each of `l`'s uses ends at the vertex the next begins at, round to the first. Where one does not, the
/// checker of the links reports it, and the loop is not traced.
fn uses_meet(model: &Model, l: View<'_, Loop>) -> bool {
    let ends: Option<Vec<(Id<Vertex>, Id<Vertex>)>> =
        l.uses().into_iter().map(|u| view(model, u).and_then(View::<EdgeUse>::vertices)).collect();
    ends.is_some_and(|ends| ends.iter().zip(ends.iter().cycle().skip(1)).all(|(a, b)| a.1 == b.0))
}

/// The report of a loop that leaves its chart.
fn outside(off: &Off, entities: &[EntityRef]) -> Report {
    let name = if off.param == 0 { "u" } else { "v" };
    let q = off.chart.quantity;
    failed(Reason::OutsideSheet, entities)
        .with_value(NamedValue::new(name, q, off.value))
        .with_value(NamedValue::new("low", q, off.chart.lo))
        .with_value(NamedValue::new("high", q, off.chart.hi))
        .with_value(NamedValue::new("sheet", Quantity::Count, f64::from(off.chart.sheet)))
}

/// Each other loop of a face is strictly inside its outer loop, and the outer loop's witness strictly outside each
/// of them: so a hole is not its own outer loop's reversal (S1 item 3).
fn nesting<'a>(model: &'a Model, f: View<'a, Face>, out: &mut Vec<Report>) {
    let loops: Vec<View<'a, Loop>> =
        f.loops().into_iter().filter_map(|l| view(model, l)).filter(|l| l.face() == f.id()).collect();
    let Some((outer, others)) = loops.split_first() else { return };
    let closed = |l: View<'a, Loop>| Trace::of(model, l).ok().filter(|t| t.open().is_none());
    let Some(around) = others.first().and_then(|_| closed(*outer)) else { return };
    let band = model.tolerance.band().millimetres();
    for &other in others {
        let Some(inner) = closed(other) else { continue };
        let (held, clear) = (around.place(other.witness(), band), inner.place(outer.witness(), band));
        if !(held.inside(band) && clear.outside(band)) {
            let windings = [("inner-winding", held.winding), ("outer-winding", clear.winding)];
            let report = failed(Reason::LoopsNotNested, &[f.id().entity(), outer.id().entity(), other.id().entity()]);
            let report = windings.into_iter().fold(report, |report, (name, winding)| match winding {
                Some(w) => report.with_value(NamedValue::new(name, Quantity::Count, f64::from(w))),
                None => report,
            });
            out.push(report.with_tolerance_in_force(band));
        }
    }
}

/// Measures the deviation of every edge whose curve or faces' surfaces the edit may have moved, from each face that
/// uses it, and stores it where it differs from what the record holds: every edge it created or changed, the edges of
/// the uses and faces it created or changed, and the edges on a curve or surface it replaced in place. Gives one report
/// for each edge whose distance from a face could not be measured, which commit refuses.
pub(crate) fn measure(edit: &mut Edit) -> Vec<Report> {
    let touched: Vec<EntityRef> = edit.created.iter().chain(&edit.changed).copied().collect();
    let next = &edit.next;
    let used =
        touched.iter().filter_map(|&u| id::<EdgeUse>(u)).filter_map(|u| view(next, u)).map(View::<EdgeUse>::edge);
    let faced =
        touched.iter().filter_map(|&f| id::<Face>(f)).filter_map(|f| view(next, f)).flat_map(View::<Face>::edges);
    let edges: BTreeSet<Id<Edge>> =
        touched.iter().filter_map(|&e| id::<Edge>(e)).chain(used).chain(faced).chain(on_replaced(edit)).collect();
    let (mut measured, mut failures) = (Vec::new(), Vec::new());
    for e in edges.into_iter().filter_map(|e| view(next, e)) {
        match deviation(next, e) {
            Ok(d) if e.deviation().millimetres().to_bits() != d.to_bits() => measured.push((e.id(), d)),
            Ok(_) => {}
            Err(miss) => failures.push(
                failed(Reason::DeviationNotMeasured, &[e.id().entity(), miss.face.entity()])
                    .with_value(NamedValue::new("parameter", miss.quantity, miss.t)),
            ),
        }
    }
    for (e, deviation) in measured {
        if let Some(record) = Edge::table_mut(&mut edit.next).live_mut(e.slot(), e.generation()) {
            record.deviation = Length::from_millimetres(deviation);
            if !edit.created.contains(&e.entity()) {
                edit.changed.insert(e.entity());
            }
        }
    }
    failures
}

/// The edges on a curve, or of a face on a surface, that the edit replaced in place. Finding them reads every edge or
/// face, which only an edit replacing geometry in place pays.
fn on_replaced(edit: &Edit) -> Vec<Id<Edge>> {
    let next = &edit.next;
    let mut found = Vec::new();
    for &g in edit.changed.iter().filter(|g| matches!(g.kind, EntityKind::Curve | EntityKind::Surface)) {
        let on_curve = Edge::table(next).records().filter(|(_, _, r)| r.curve.entity() == g);
        found.extend(on_curve.map(|(slot, generation, _)| Id::<Edge>::new(slot, generation)));
        let faces = Face::table(next).records().filter(|(_, _, r)| r.surface.entity() == g);
        let faces: Vec<Id<Face>> = faces.map(|(slot, generation, _)| Id::new(slot, generation)).collect();
        found.extend(faces.into_iter().filter_map(|f| view(next, f)).flat_map(View::<Face>::edges));
    }
    found
}

/// Where an edge's distance from a face could not be measured: the face, and the parameter of the edge's curve.
struct Unmeasured {
    face: Id<Face>,
    t: f64,
    quantity: Quantity,
}

/// The largest distance from `e`'s curve, at even steps over its range, to the surface of each face that uses it;
/// zero for an edge no face uses.
fn deviation(model: &Model, e: View<'_, Edge>) -> Result<f64, Unmeasured> {
    let Some(curve) = e.curve() else { return Ok(0.0) };
    let (mut worst, mut scratch) = (0.0, Scratch::default());
    for u in e.uses().into_iter().filter_map(|u| view(model, u)).filter(|u| u.edge() == e.id()) {
        let UseOwner::Loop(owner) = u.owner() else { continue };
        let Some(face) = view(model, owner).and_then(|l| view(model, l.face())) else { continue };
        let Some(surface) = face.surface() else { continue };
        for k in 0..=STEPS {
            let t = step(e.range(), k);
            let seed = u.pcurve().map(|p| p.uv(t));
            let Ok(found) = surface.closest(curve.point(t), seed, model.tolerance, &mut scratch) else {
                return Err(Unmeasured { face: face.id(), t, quantity: curve.range().quantity() });
            };
            let d = found.distance.millimetres();
            if d > worst {
                worst = d;
            }
        }
    }
    Ok(worst)
}

#[cfg(test)]
mod tests;
