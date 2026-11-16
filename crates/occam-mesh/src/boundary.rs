//! Source: original
//! A face's boundary in its parameters: each loop through its uses in order, each use through its edge's samples in
//! its own direction, each sample placed by the use's curve in the face's parameters at the sample's parameter on the
//! edge, and holding the sample's position bit for bit. Where one use ends and the next begins at one vertex, the two
//! are one point if their places agree to within 2^-32 of the boundary's larger span, or of its size, as two curves
//! through one vertex agree to their rounding. Where they are further apart, as where a loop crosses a pole, an apex or
//! a collapsed row from one side of its face to the other, both points are kept and a segment joins them: both hold the
//! vertex's position, so the triangle on that segment has two corners at one place and is left out of the mesh.
//!
//! Each segment knows the piece of an edge it runs along, between two of its samples, which repair cuts where the
//! boundary collides ([`crate::repair`]); a segment that joins two places of one vertex is no piece.

use std::ops::Range;

use occam_report::{Code, Fault, Reason, Report};
use occam_topo::{Edge, EdgeUse, Face, Id, Loop, Snapshot, View};
use occam_units::{Point, UvPoint, World};

use crate::edges::{Edges, stale};
use crate::faces::Side;
use crate::limits::{higher, lower};
use crate::triangulation::Fail;

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
    /// The piece of an edge each segment runs along; none where it joins two uses, as across a pole.
    pub pieces: Vec<Option<Piece>>,
    /// Each use, in loop order, with its samples' points in its direction.
    pub sides: Vec<Side>,
    /// The first loop that joins up to fewer than three points, so that it bounds nothing: its segments, and the edges
    /// of its uses.
    pub short: Option<(Range<usize>, Vec<Id<Edge>>)>,
    /// The piece each point is reached by from the point before it round its loop.
    arrive: Vec<Option<Piece>>,
}

/// A piece of an edge, between two of its samples next to each other: the edge, and the samples' parameters on it,
/// the lower first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Piece {
    pub edge: Id<Edge>,
    pub from: f64,
    pub to: f64,
}

impl Piece {
    /// The piece of `edge` between its samples at `a` and `b`, either way round.
    fn of(edge: Id<Edge>, a: f64, b: f64) -> Self {
        Self { edge, from: lower(a, b), to: higher(a, b) }
    }
}

/// One sample of a use, before its loop is joined up.
#[derive(Clone, Copy)]
struct Raw {
    uv: UvPoint,
    at: Point<World>,
    /// The sample's parameter on its edge.
    t: f64,
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
    let raw = |(&t, &at): (&f64, &Point<World>)| Raw { uv: pcurve.uv(t), at, t };
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

/// How far apart two places of one vertex may be in each parameter and still be one point: 2^-32 of the larger of the
/// spans of `places` and of how far from nought they reach. A curve's rounding is of the size of its numbers, not of how
/// far the face reaches across them: the catalogue's crescent 1e-4 wide has a tip whose places, one through each arc,
/// agree to 1.4e-13 across a span of 3.7e-5, and kept apart, the arcs' chords crossed there however finely cut. Two
/// places a loop crosses a pole between are the face's width apart, far beyond this.
fn closeness(places: impl Iterator<Item = UvPoint>) -> [f64; 2] {
    let (mut lo, mut hi, mut size) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2], 0.0);
    for uv in places {
        lo = [lower(lo[0], uv.u()), lower(lo[1], uv.v())];
        hi = [higher(hi[0], uv.u()), higher(hi[1], uv.v())];
        size = higher(size, higher(uv.u().abs(), uv.v().abs()));
    }
    let fraction = 1.0 / 4_294_967_296.0;
    [higher(higher(hi[0] - lo[0], hi[1] - lo[1]), size) * fraction; 2]
}

/// The boundary of `face` against the samples of `edges`, each segment with the piece of an edge it runs along, and the
/// first loop that bounds nothing, if one does.
///
/// # Errors
///
/// [`Code::FaceNotMeshed`] where an edge was refused, as [`Reason::EdgeNotSampled`] with the edge's remedies; and where
/// a use has no curve in the face's parameters, as [`Reason::NoParamCurve`].
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
                    boundary.push(start, side.edge, None);
                }
                side.vertices.push(boundary.last());
                opening = opening.or(Some(start));
                previous = Some(start);
            }
            for raw in raws {
                let piece = previous.map(|before| Piece::of(side.edge, before.t, raw.t));
                boundary.push(raw, side.edge, piece);
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
        boundary.close(first, sides);
    }
    Ok(boundary)
}

impl Boundary {
    /// The number of the last point, as a mesh's vertex.
    fn last(&self) -> u32 {
        u32::try_from(self.uv.len().saturating_sub(1)).unwrap_or(u32::MAX)
    }

    /// Adds the point `raw` stands for, a sample of `edge`, reached by `piece` from the point before it.
    fn push(&mut self, raw: Raw, edge: Id<Edge>, piece: Option<Piece>) {
        self.uv.push(raw.uv);
        self.at.push(raw.at);
        self.from.push(edge);
        self.arrive.push(piece);
    }

    /// Makes the last point the loop's first, at `first`, for the uses of the loop from `sides` on: the piece that
    /// reached the last now reaches the first.
    fn fold_last(&mut self, first: usize, sides: usize) {
        let end = u32::try_from(self.uv.len().saturating_sub(1)).unwrap_or(u32::MAX);
        let into = u32::try_from(first).unwrap_or(u32::MAX);
        self.uv.pop();
        self.at.pop();
        self.from.pop();
        let closing = self.arrive.pop().flatten();
        if let Some(slot) = self.arrive.get_mut(first) {
            *slot = closing;
        }
        for side in self.sides.iter_mut().skip(sides) {
            side.vertices.iter_mut().filter(|v| **v == end).for_each(|v| *v = into);
        }
    }

    /// Joins the loop whose points start at `first`, and whose uses at `sides`, by segments, each with the piece it
    /// runs along; and marks it short if it has fewer than three points.
    fn close(&mut self, first: usize, sides: usize) {
        let end = self.uv.len();
        let after = |k: usize| if k.saturating_add(1) == end { first } else { k.saturating_add(1) };
        let segments = self.segments.len();
        for k in first..end {
            self.segments.push([k, after(k)]);
            self.pieces.push(self.arrive.get(after(k)).copied().flatten());
        }
        if end.saturating_sub(first) < 3 && self.short.is_none() {
            let edges = self.sides.iter().skip(sides).map(|side| side.edge).collect();
            self.short = Some((segments..self.segments.len(), edges));
        }
    }

    /// The pieces the segments between points `a` and `b` run along, either way round.
    fn between(&self, a: usize, b: usize) -> impl Iterator<Item = Piece> + '_ {
        let joins = move |&(&[p, q], _): &(&[usize; 2], &Option<Piece>)| (p == a && q == b) || (p == b && q == a);
        self.segments.iter().zip(&self.pieces).filter(joins).filter_map(|(_, piece)| *piece)
    }

    /// The pieces of edges that `fail` involves, which finer samples may part: the two segments that cross, or the one
    /// a point lies on; none for a failure that finer samples cannot cure, as two points at one place.
    pub(crate) fn collided(&self, fail: Fail) -> Vec<Piece> {
        match fail {
            Fail::Crosses([a, b], [c, d]) => self.between(a, b).chain(self.between(c, d)).collect(),
            Fail::Touches([a, b], _) => self.between(a, b).collect(),
            Fail::Coincide(..) | Fail::Undecided | Fail::Broken => Vec::new(),
        }
    }

    /// The edges `fail` involves, which the report refusing the face names, in id order: those whose pieces collided,
    /// the edge of each point it names, and of each end of a segment that is no piece.
    pub(crate) fn involved(&self, fail: Fail) -> Vec<Id<Edge>> {
        let of = |p: usize| self.from.get(p).copied();
        let along = |a: usize, b: usize| {
            let pieces: Vec<Id<Edge>> = self.between(a, b).map(|piece| piece.edge).collect();
            if pieces.is_empty() { [of(a), of(b)].into_iter().flatten().collect() } else { pieces }
        };
        let mut edges: Vec<Id<Edge>> = match fail {
            Fail::Crosses([a, b], [c, d]) => along(a, b).into_iter().chain(along(c, d)).collect(),
            Fail::Touches([a, b], c) => along(a, b).into_iter().chain(of(c)).collect(),
            Fail::Coincide(a, b) => [of(a), of(b)].into_iter().flatten().collect(),
            Fail::Undecided | Fail::Broken => Vec::new(),
        };
        edges.sort_unstable();
        edges.dedup();
        edges
    }

    /// The pieces of the short loop's segments, if a loop is short.
    pub(crate) fn short_pieces(&self) -> Option<Vec<Piece>> {
        let (segments, _) = self.short.as_ref()?;
        Some(self.pieces.get(segments.clone()).unwrap_or_default().iter().filter_map(|piece| *piece).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_places_of_one_vertex_are_one_point_within_2_to_the_minus_32_of_the_larger_span_or_size() {
        let near = closeness([UvPoint::new(-1.0, 3.0), UvPoint::new(1.0, -5.0)].into_iter());
        assert_eq!(near, [8.0 / 4_294_967_296.0; 2]);
        // The catalogue's crescent 1e-4 wide, between arcs of radius 10 and 500: the places of its tip, one through
        // each arc's curve, agree to 1.4e-13, which is the far arc's rounding, across a span of 3.7e-5.
        let thin = closeness(
            [UvPoint::new(0.0, 0.0), UvPoint::new(0.081_315_711_261_473_23, 3.673_500_125_422_890_5e-5)].into_iter(),
        );
        assert!(thin[1] > 1.4e-13, "{thin:?}");
        let far = closeness([UvPoint::new(1000.0, -1000.0), UvPoint::new(1001.0, -1000.0)].into_iter());
        assert_eq!(far, [1001.0 / 4_294_967_296.0; 2], "as far from nought as the places reach");
    }
}
