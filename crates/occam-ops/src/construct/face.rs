//! Source: original
//! The `face` operation (PARITY 1.6): a planar face bounded by a closed wire, with holes, as a sheet of one face. Each
//! wire is an earlier step's, which the face takes, so its edges and vertices keep their ids, or one written in place.
//!
//! The face lies in the plane its outer wire runs anticlockwise about, and every wire must lie in it within the band.
//! Its loops may not touch or cross, each hole lies inside the outer wire and outside the other holes, and every loop
//! has a witness: a point strictly inside it, and for the outer loop outside every hole, clear of every edge.

use std::collections::BTreeSet;

use occam_report::{Code, EntityKind, EntityRef, Fault, Input, NamedValue, Remedy, RemedyKind, Report};
use occam_topo::{Body, Edge, EdgeUse, Edit, Id, Snapshot, Vertex, Wire};
use occam_units::{UvPoint, Vec3};

use super::build::{self, Ring, refused, shown};
use super::flat::{Flat, area};
use super::given::{FaceArgs, WireRef};
use super::loops::Loops;
use super::outline::{Run, not_a_wire, off_plane, plane_through};
use super::space::Flatland;
use super::touch::{Touch, first_touch};
use super::wire::{band, crosses_itself, made};
use crate::operand::{Labels, Operand};
use crate::read::count;

/// How far, in bands, a loop's witness must be from every edge of its face.
const CLEAR: f64 = 16.0;

/// Makes the face `args` asks for in `edit`, and gives it.
///
/// # Errors
///
/// What reading each wire reports, and a report for a face whose wires do not bound one: not in one plane, touching or
/// crossing, a hole outside the outer wire or inside another hole, or a loop too thin to hold a witness.
pub(crate) fn face(edit: &mut Edit, args: &FaceArgs, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let band = band(edit);
    let mut runs: Vec<(Run, Input)> = Vec::new();
    for wire in core::iter::once(&args.outer).chain(&args.holes) {
        runs.push(loop_of(edit, wire, labels, band)?);
    }
    let (plane, runs) = planar(runs, band)?;
    let flats: Vec<Vec<Flat>> = runs.iter().map(|(run, _)| run.flats(&plane)).collect();
    let loops = Loops::new(&flats);
    apart(&plane, &runs, (&flats, &loops), band)?;
    let mut rings = Vec::new();
    for (k, (run, at)) in runs.iter().enumerate() {
        let clear = CLEAR * band;
        let (uv, _) = loops
            .witness(k, clear)
            .ok_or_else(|| refused(Code::NoInterior, at, band).with_value(NamedValue::length("clearance", clear)))?;
        rings.push(Ring { edges: run.edges.clone(), witness: UvPoint::new(uv[0], uv[1]) });
    }
    let bents: Vec<_> = runs.iter().map(|(run, _)| run.bents.clone()).collect();
    Ok(Some(build::sheet(edit, &plane, &rings, &bents)?.entity()))
}

/// The plane of the outer wire, facing so that it runs anticlockwise, which every wire must lie in, and the runs with
/// each hole turned to run clockwise.
///
/// # Errors
///
/// [`Code::NoInterior`] for an outer wire whose points span no plane beyond the band, and [`Code::NotPlanar`] for a
/// wire that leaves the plane.
fn planar(runs: Vec<(Run, Input)>, band: f64) -> Result<(Flatland, Vec<(Run, Input)>), Report> {
    let Some((outer, at)) = runs.first() else { return Err(refused(Code::NoInterior, &Input::at("outer"), band)) };
    let plane = plane_through(&outer.samples(), band)
        .ok_or_else(|| refused(Code::NoInterior, at, band).with_value(NamedValue::length("clearance", 0.0)))?;
    for (run, at) in &runs {
        if let Some((point, distance)) = off_plane(&plane, &run.samples(), band) {
            return Err(refused(Code::NotPlanar, at, band)
                .with_value(shown("point", point))
                .with_value(NamedValue::length("distance", distance)));
        }
    }
    let plane = if turning(outer, &plane) < 0.0 { plane.flipped() } else { plane };
    let turned = runs
        .into_iter()
        .enumerate()
        .map(|(k, (run, at))| if k > 0 && turning(&run, &plane) > 0.0 { (run.reversed(), at) } else { (run, at) })
        .collect();
    Ok((plane, turned))
}

/// Twice the area `run` bounds in `plane`, anticlockwise positive.
fn turning(run: &Run, plane: &Flatland) -> f64 {
    let flats = run.flats(plane);
    let length: f64 = flats.iter().map(|f| f.length()).sum();
    area(&flats, length * 1e-3)
}

/// That the loops neither touch nor cross, each hole lies inside the outer loop, and no hole lies inside another.
///
/// # Errors
///
/// [`Code::WireCrossesItself`], naming the edges that touch in the order the loop's edges were given in, whether or not
/// it was turned, [`Code::WiresCross`], [`Code::HoleOutside`] or [`Code::HolesNested`].
fn apart(
    plane: &Flatland,
    runs: &[(Run, Input)],
    (flats, loops): (&[Vec<Flat>], &Loops<'_>),
    band: f64,
) -> Result<(), Report> {
    let at = |k: usize| runs.get(k).map_or_else(|| Input::at("outer"), |(_, at)| at.clone());
    let closed: Vec<(&[Flat], bool)> = flats.iter().map(|f| (f.as_slice(), true)).collect();
    if let Some(touch) = first_touch(&closed, band) {
        let (first, second) = (touch.first.0, touch.second.0);
        if first == second {
            let given = |k: usize| runs.get(first).map_or(k, |(run, _)| run.given(k));
            let (a, b) = (given(touch.first.1), given(touch.second.1));
            let touch = Touch { first: (first, a.min(b)), second: (first, a.max(b)), at: touch.at };
            return Err(crosses_itself(plane, touch, &at(first), band));
        }
        return Err(refused(Code::WiresCross, &at(second), band)
            .with_value(shown("point", plane.point(touch.at)))
            .with_value(count("other", first)));
    }
    for (k, hole) in flats.iter().enumerate().skip(1) {
        let start = hole.first().map_or([0.0, 0.0], |piece| piece.start());
        if !loops.inside(0, k) {
            return Err(refused(Code::HoleOutside, &at(k), band).with_value(shown("point", plane.point(start))));
        }
        if let Some(j) = loops.nested(k) {
            return Err(refused(Code::HolesNested, &at(k), band).with_value(count("other", j)));
        }
    }
    Ok(())
}

/// The closed run of edges `wire` names, read back: made here for a wire written in place, or taken from an earlier
/// step's wire or the model's, whose body and uses go so that its edges bound the face instead.
///
/// # Errors
///
/// [`Code::WireOpen`] for a wire that does not close, what making a wire written in place reports, and for a named wire,
/// [`Code::InputUnknownLabel`], [`Code::StaleId`] or [`Code::NotAWire`].
fn loop_of(edit: &mut Edit, wire: &WireRef, labels: &Labels, band: f64) -> Result<(Run, Input), Report> {
    match wire {
        WireRef::Given(args) => {
            let pieces = made(args, band)?;
            let ends = pieces.first().map(|(m, _)| m.start).zip(pieces.last().map(|(m, _)| m.end));
            let chain = build::chain(edit, pieces, band)?;
            if let (false, Some((start, end))) = (chain.closed, ends) {
                return Err(open(args.at(), start, end, band));
            }
            let edges = chain.edges.iter().map(|&e| (e, false)).collect();
            Ok((Run::read(edit, edges, args.at())?, args.at().clone()))
        }
        WireRef::Named(operand, at) => {
            let id = named(edit.base(), operand, labels, at)?;
            let edges = take(edit, id, at, band)?;
            Ok((Run::read(edit, edges, at)?, at.clone()))
        }
    }
}

/// The wire at `at` runs from `start` to `end`, which are further apart than `band`.
fn open(at: &Input, start: Vec3, end: Vec3, band: f64) -> Report {
    refused(Code::WireOpen, at, band)
        .with_value(NamedValue::length("gap", (end - start).norm()))
        .with_value(shown("start", start))
        .with_remedy(Remedy::new(RemedyKind::MeetAt).with_value(shown("point", start)))
}

/// The wire `operand`, at `at`, names: an earlier step's, by its label, or the model's, by the id of its body or of the
/// wire itself.
///
/// # Errors
///
/// [`Code::InputUnknownLabel`] for a label no earlier step has, [`Code::StaleId`] for an id that names no live entity,
/// and [`Code::NotAWire`] for anything but a wire body or a wire, naming what it names so that it is described.
fn named(model: &Snapshot, operand: &Operand, labels: &Labels, at: &Input) -> Result<Id<Wire>, Report> {
    let entity = match operand {
        Operand::Label(label) => labels.get(label).ok_or_else(|| {
            Report::new(Code::InputUnknownLabel, Fault::InvalidInput).with_input(at.clone().given(label))
        })?,
        Operand::Entity(entity) => *entity,
    };
    let placed = |report: Report| report.with_input(at.clone());
    match entity.kind {
        EntityKind::Wire => model.resolve::<Wire>(entity).map_err(placed),
        EntityKind::Body => {
            let body = model.resolve::<Body>(entity).map_err(placed)?;
            let first = model.view(body).and_then(|b| b.wires().first().copied());
            first.ok_or_else(|| not_a_wire(at, &entity.text()).with_entity(entity))
        }
        _ => Err(not_a_wire(at, &entity.text()).with_entity(entity)),
    }
}

/// Takes the wire `id` and its body, of which it must be the only wire, and gives its edges in order, each with whether
/// the wire runs against it.
///
/// # Errors
///
/// [`Code::WireOpen`] if the wire does not close, [`Code::NotAWire`] if its body has other wires, and [`Code::StaleId`]
/// if a record it reads is gone.
fn take(edit: &mut Edit, id: Id<Wire>, at: &Input, band: f64) -> Result<Vec<(Id<Edge>, bool)>, Report> {
    let gone =
        |entity: EntityRef| Report::new(Code::StaleId, Fault::InvalidInput).with_entity(entity).with_input(at.clone());
    let wire = edit.record(id).ok_or_else(|| gone(id.entity()))?;
    let (mut uses, mut seen, mut at_use) = (Vec::<Id<EdgeUse>>::new(), BTreeSet::new(), Some(wire.first));
    while let Some(u) = at_use.filter(|&u| seen.insert(u)) {
        uses.push(u);
        at_use = edit.record(u).ok_or_else(|| gone(u.entity()))?.next;
    }
    let records: Vec<_> = uses.iter().filter_map(|&u| edit.record(u)).collect();
    let edges: Vec<(Id<Edge>, bool)> = records.iter().map(|r| (r.edge, r.reversed)).collect();
    if at_use.is_none() {
        let corner = |e: Id<Edge>, last: bool| -> Option<Vec3> {
            let (start, finish) = edit.record(e)?.ends;
            Some(edit.record::<Vertex>(if last { finish } else { start })?.point.to_vec3())
        };
        let (first, last) = (edges.first().copied(), edges.last().copied());
        let nowhere = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        let start = first.and_then(|(e, back)| corner(e, back)).unwrap_or(nowhere);
        let end = last.and_then(|(e, back)| corner(e, !back)).unwrap_or(nowhere);
        return Err(open(at, start, end, band));
    }
    let only = edit.record(wire.body).is_some_and(|b| b.first_wire == Some(id)) && wire.next.is_none();
    if !only {
        return Err(not_a_wire(at, "a wire of a body with others"));
    }
    for &u in &uses {
        edit.remove(u)?;
    }
    edit.remove(id)?;
    edit.remove(wire.body)?;
    Ok(edges)
}

#[cfg(test)]
mod tests;
