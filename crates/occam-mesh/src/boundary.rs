//! Source: original
//! A face's boundary in its parameters: each loop through its uses in order, each use through its edge's samples in
//! its own direction, each sample placed by the use's curve in the face's parameters at the sample's parameter on the
//! edge, and holding the sample's position bit for bit. Where one use ends and the next begins at one vertex, the two
//! are one point if their places agree to within 2^-32 of the boundary's span in each parameter, as two curves through
//! one vertex agree to their rounding. Where they are further apart, as where a loop crosses a pole, an apex or a
//! collapsed row from one side of its face to the other, both points are kept and a segment joins them: both hold the
//! vertex's position, so the triangle on that segment has two corners at one place and is left out of the mesh.

use occam_report::{Code, Fault, Reason, Report};
use occam_topo::{Edge, EdgeUse, Face, Id, Loop, Snapshot, View};
use occam_units::{Point, UvPoint, World};

use crate::edges::{Edges, stale};
use crate::faces::Side;
use crate::limits::{higher, lower};

/// A face's boundary in its parameters.
#[derive(Default)]
pub(crate) struct Boundary {
    /// Each point's place in the face's parameters.
    pub uv: Vec<UvPoint>,
    /// Each point's position: its edge's sample, bit for bit.
    pub at: Vec<Point<World>>,
    /// The edge each point came from, which a report names.
    pub from: Vec<Id<Edge>>,
    /// The segments between neighbouring points round each loop.
    pub segments: Vec<[usize; 2]>,
    /// Each use, in loop order, with its samples' points in its direction.
    pub sides: Vec<Side>,
}

/// One sample of a use, before its loop is joined up.
#[derive(Clone, Copy)]
struct Raw {
    uv: UvPoint,
    at: Point<World>,
}

/// The report that `face` could not be meshed, for `reason`, naming `edges`.
pub(crate) fn not_meshed(face: Id<Face>, reason: Reason, fault: Fault, edges: &[Id<Edge>]) -> Report {
    let report = Report::new(Code::FaceNotMeshed, fault).with_reason(reason).with_entity(face.entity());
    edges.iter().fold(report, |report, edge| report.with_entity(edge.entity()))
}

/// The samples of use `u`, in its direction.
fn samples(model: &Snapshot, face: Id<Face>, u: Id<EdgeUse>, edges: &Edges) -> Result<(Side, Vec<Raw>), Report> {
    let used = model.view(u).ok_or_else(|| stale(model, u))?;
    let edge = used.edge();
    let sampled = match edges.get(edge) {
        Some(Ok(sampled)) => sampled,
        Some(Err(refused)) => {
            let report = not_meshed(face, Reason::EdgeNotSampled, Fault::InvalidInput, &[edge]);
            return Err(refused.remedies.iter().fold(report, |report, remedy| report.with_remedy(remedy.clone())));
        }
        None => return Err(not_meshed(face, Reason::EdgeNotSampled, Fault::Internal, &[edge])),
    };
    let Some(pcurve) = used.pcurve() else {
        return Err(not_meshed(face, Reason::NoParamCurve, Fault::NotImplemented, &[edge]));
    };
    let raw = |(&t, &at): (&f64, &Point<World>)| Raw { uv: pcurve.uv(t), at };
    let mut raws: Vec<Raw> = sampled.params.iter().zip(&sampled.points).map(raw).collect();
    if used.reversed() {
        raws.reverse();
    }
    Ok((Side { edge, reversed: used.reversed(), vertices: Vec::new() }, raws))
}

/// A loop's uses, each with its samples.
type Uses = Vec<(Side, Vec<Raw>)>;

/// Each loop of `face`, each use with its samples.
fn loops(model: &Snapshot, face: View<'_, Face>, edges: &Edges) -> Result<Vec<Uses>, Report> {
    let of_loop = |l: Id<Loop>| {
        let uses = model.view(l).ok_or_else(|| stale(model, l))?.uses();
        uses.into_iter().map(|u| samples(model, face.id(), u, edges)).collect::<Result<Vec<_>, Report>>()
    };
    face.loops().into_iter().map(of_loop).collect()
}

/// How far apart two places of one vertex may be in each parameter and still be one point: 2^-32 of the span of
/// `places` in it.
fn closeness(places: impl Iterator<Item = UvPoint>) -> [f64; 2] {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for uv in places {
        lo = [lower(lo[0], uv.u()), lower(lo[1], uv.v())];
        hi = [higher(hi[0], uv.u()), higher(hi[1], uv.v())];
    }
    let fraction = 1.0 / 4_294_967_296.0;
    [(hi[0] - lo[0]) * fraction, (hi[1] - lo[1]) * fraction]
}

/// The boundary of `face` against the samples of `edges`.
///
/// # Errors
///
/// [`Code::FaceNotMeshed`] where an edge was refused, as [`Reason::EdgeNotSampled`] with the edge's remedies; where a
/// use has no curve in the face's parameters, as [`Reason::NoParamCurve`]; and where a loop joins up to fewer than
/// three points, so that it bounds nothing, as [`Reason::BoundaryCrosses`].
pub(crate) fn of(model: &Snapshot, face: View<'_, Face>, edges: &Edges) -> Result<Boundary, Report> {
    let loops = loops(model, face, edges)?;
    let near = closeness(loops.iter().flatten().flat_map(|(_, raws)| raws.iter().map(|r| r.uv)));
    let same = |a: &Raw, b: &Raw| (a.uv.u() - b.uv.u()).abs() <= near[0] && (a.uv.v() - b.uv.v()).abs() <= near[1];
    let mut boundary = Boundary::default();
    for uses in loops {
        let (first, sides) = (boundary.uv.len(), boundary.sides.len());
        let (mut opening, mut previous): (Option<Raw>, Option<Raw>) = (None, None);
        for (mut side, raws) in uses {
            let mut raws = raws.into_iter();
            // A use begins at the vertex the use before it ends at: one point there where their places agree.
            if let Some(start) = raws.next() {
                if !previous.is_some_and(|end| same(&end, &start)) {
                    boundary.push(start, side.edge);
                }
                side.vertices.push(boundary.last());
                opening = opening.or(Some(start));
                previous = Some(start);
            }
            for raw in raws {
                boundary.push(raw, side.edge);
                side.vertices.push(boundary.last());
                previous = Some(raw);
            }
            boundary.sides.push(side);
        }
        if let (Some(opening), Some(last)) = (opening, previous)
            && same(&opening, &last)
        {
            boundary.fold_last(first, sides);
        }
        let end = boundary.uv.len();
        if end.saturating_sub(first) < 3 {
            let here: Vec<Id<Edge>> = boundary.sides.iter().skip(sides).map(|side| side.edge).collect();
            return Err(not_meshed(face.id(), Reason::BoundaryCrosses, Fault::InvalidInput, &here));
        }
        let after = |k: usize| if k.saturating_add(1) == end { first } else { k.saturating_add(1) };
        boundary.segments.extend((first..end).map(|k| [k, after(k)]));
    }
    Ok(boundary)
}

impl Boundary {
    /// The number of the last point, as a mesh's vertex.
    fn last(&self) -> u32 {
        u32::try_from(self.uv.len().saturating_sub(1)).unwrap_or(u32::MAX)
    }

    /// Adds the point `raw` stands for, a sample of `edge`.
    fn push(&mut self, raw: Raw, edge: Id<Edge>) {
        self.uv.push(raw.uv);
        self.at.push(raw.at);
        self.from.push(edge);
    }

    /// Makes the last point the loop's first, at `first`, for the uses of the loop from `sides` on.
    fn fold_last(&mut self, first: usize, sides: usize) {
        let end = u32::try_from(self.uv.len().saturating_sub(1)).unwrap_or(u32::MAX);
        let into = u32::try_from(first).unwrap_or(u32::MAX);
        self.uv.pop();
        self.at.pop();
        self.from.pop();
        for side in self.sides.iter_mut().skip(sides) {
            side.vertices.iter_mut().filter(|v| **v == end).for_each(|v| *v = into);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_places_of_one_vertex_are_one_point_within_2_to_the_minus_32_of_the_span_in_each_parameter() {
        let near = closeness([UvPoint::new(-1.0, 3.0), UvPoint::new(1.0, -5.0)].into_iter());
        assert_eq!(near, [2.0 / 4_294_967_296.0, 8.0 / 4_294_967_296.0]);
    }
}
