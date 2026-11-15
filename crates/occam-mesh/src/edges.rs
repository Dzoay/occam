//! Source: original
//! Every edge any face uses, sampled once (P9's phase 1), so that the faces either side of it are meshed against the
//! same points and cannot disagree along it. Each face first reads how sharply its surface bends, one face a task; each
//! edge then takes the step the most sharply bent of its faces asks, never below the floor, one edge a task.
//!
//! An edge is cut by halving, depth first, until every piece meets the limits at its middle: no longer than the step,
//! its middle within half the deflection of its chord, leaving the faces the other half as P9 did, and turning through
//! no more than half the angle either side of it. A piece no longer than twice the floor is never cut. A closed edge
//! starts as four pieces and an edge whose parameter is an angle as pieces of at most a quarter turn, so that neither
//! is ever drawn as a chord with no width. An edge that would need more samples than the cap is refused, and the
//! report gives the first deflection, doubling from the one asked, at which it fits.

use core::f64::consts::FRAC_PI_2;
use std::collections::BTreeMap;
use std::sync::Arc;

use occam_geom::Curve;
use occam_math::{Exec, Grain};
use occam_report::{Code, Fault, NamedValue, Remedy, RemedyKind, Report};
use occam_topo::{Edge, Face, Id, Kind, Snapshot, View};
use occam_units::{Length, ParamKind, Point, Vec3, World};

use crate::curvature;
use crate::limits::{Limits, higher, lower};

/// One edge's samples, which every face that uses it is meshed against.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSamples {
    /// Where they are on the edge's curve, rising from the start of the edge's range to its end, both exactly.
    pub params: Vec<f64>,
    /// The points there: the curve's, except at the ends, which are the edge's vertices', so that every edge meeting at
    /// a vertex ends at the same bits.
    pub points: Vec<Point<World>>,
}

/// Every edge any face of a model uses, each with its samples or the report refusing it, in id order.
#[derive(Clone, Debug, PartialEq)]
pub struct Edges {
    list: Vec<(Id<Edge>, Result<EdgeSamples, Report>)>,
}

impl Edges {
    /// The samples of `edge`, or the report refusing it; `None` for an edge no face uses.
    #[must_use]
    pub fn get(&self, edge: Id<Edge>) -> Option<Result<&EdgeSamples, &Report>> {
        let at = self.list.binary_search_by_key(&edge, |(id, _)| *id).ok()?;
        self.list.get(at).map(|(_, result)| result.as_ref())
    }

    /// Each edge with its samples or the report refusing it, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<Edge>, Result<&EdgeSamples, &Report>)> + '_ {
        self.list.iter().map(|(id, result)| (*id, result.as_ref()))
    }

    /// The reports refusing edges, in id order: none if every edge was sampled.
    pub fn refused(&self) -> impl Iterator<Item = &Report> + '_ {
        self.list.iter().filter_map(|(_, result)| result.as_ref().err())
    }
}

/// Samples every edge any face of `model` uses, once, on `exec`, each as finely as the finest of its faces asks within
/// `limits`; the same bits at any thread count. An edge that would need more than [`Limits::SAMPLES_PER_EDGE`] samples
/// is refused with [`Code::EdgeOverCap`], naming it, with the remedy [`RemedyKind::UseDeflection`] where a deflection
/// fits, and the other edges are sampled as if it were not there.
#[must_use]
pub fn sample_edges(exec: &Exec, model: &Snapshot, limits: Limits) -> Edges {
    let model = Arc::new(model.clone());
    let faces: Arc<[Id<Face>]> = model.ids::<Face>().into();
    let (read, of) = (Arc::clone(&model), Arc::clone(&faces));
    let bends = exec.map(faces.len(), Grain::Each, move |k| of.get(k).map_or(0.0, |&f| curvature::of_face(&read, f)));
    let jobs: Arc<[(Id<Edge>, f64)]> = finest(&model, &faces, &bends).into();
    let (read, of) = (Arc::clone(&model), Arc::clone(&jobs));
    let sampled = exec.map(jobs.len(), Grain::Each, move |k| {
        of.get(k).map(|&(edge, bend)| (edge, sample_edge(&read, edge, bend, limits)))
    });
    Edges { list: sampled.into_iter().flatten().collect() }
}

/// Each edge `faces` use, in id order, with the largest curvature of the faces that use it, `bends` holding each face's.
fn finest(model: &Snapshot, faces: &[Id<Face>], bends: &[f64]) -> Vec<(Id<Edge>, f64)> {
    let mut edges: BTreeMap<Id<Edge>, f64> = BTreeMap::new();
    for (&face, &bend) in faces.iter().zip(bends) {
        for edge in model.view(face).map(View::<Face>::edges).unwrap_or_default() {
            let most = edges.entry(edge).or_insert(0.0);
            *most = higher(*most, bend);
        }
    }
    edges.into_iter().collect()
}

/// The report for `id`, which a committed model's links never leave dead.
fn stale<K: Kind>(model: &Snapshot, id: Id<K>) -> Report {
    match model.resolve::<K>(id.entity()) {
        Err(report) => report,
        Ok(_) => Report::new(Code::StaleId, Fault::Internal).with_entity(id.entity()),
    }
}

/// `edge` sampled for faces whose largest curvature is `bend`.
fn sample_edge(model: &Snapshot, edge: Id<Edge>, bend: f64, limits: Limits) -> Result<EdgeSamples, Report> {
    let view = model.view(edge).ok_or_else(|| stale(model, edge))?;
    let curve = view.curve().ok_or_else(|| stale(model, view.curve_id()))?;
    let (start, finish) = view.ends();
    let piece = Piece { curve, range: view.range(), closed: start == finish };
    match piece.cut(&Criteria::new(limits, bend)) {
        Some(samples) => {
            let at = |vertex| model.view(vertex).map(View::point);
            Ok(finished(&samples, (at(start), at(finish))))
        }
        None => Err(over_cap(edge, limits, piece.fitting(bend, limits))),
    }
}

/// The samples as the faces read them, the ends at the edge's vertices where it has them.
fn finished(samples: &[Sample], (start, finish): (Option<Point<World>>, Option<Point<World>>)) -> EdgeSamples {
    let params = samples.iter().map(|s| s.at).collect();
    let mut points: Vec<Point<World>> = samples.iter().map(|s| Point::from_vec3(s.point)).collect();
    if let (Some(first), Some(start)) = (points.first_mut(), start) {
        *first = start;
    }
    if let (Some(last), Some(finish)) = (points.last_mut(), finish) {
        *last = finish;
    }
    EdgeSamples { params, points }
}

/// The report refusing `edge` at `limits`, with `fits` as its remedy if a deflection does.
fn over_cap(edge: Id<Edge>, limits: Limits, fits: Option<Length>) -> Report {
    let report = Report::new(Code::EdgeOverCap, Fault::InvalidInput)
        .with_entity(edge.entity())
        .with_value(NamedValue::count("cap", Limits::SAMPLES_PER_EDGE))
        .with_value(NamedValue::length("deflection", limits.deflection().millimetres()));
    match fits {
        Some(deflection) => report.with_remedy(
            Remedy::new(RemedyKind::UseDeflection)
                .with_value(NamedValue::length("deflection", deflection.millimetres())),
        ),
        None => report,
    }
}

/// A point of an edge's curve with its derivative there, as plain numbers.
#[derive(Clone, Copy, Debug)]
struct Sample {
    at: f64,
    point: Vec3,
    tangent: Vec3,
}

/// What a piece is held to.
#[derive(Clone, Copy, Debug)]
struct Criteria {
    /// The farthest a piece's middle may be from its chord.
    chord: f64,
    /// The longest a piece may be: the step its faces ask.
    step: f64,
    /// A piece no longer than twice this is never cut.
    floor: f64,
    /// The cosine of the most a piece may turn either side of its middle.
    turn: f64,
}

impl Criteria {
    /// The criteria of `limits` for an edge whose faces' largest curvature is `bend`.
    fn new(limits: Limits, bend: f64) -> Self {
        Self {
            chord: 0.5 * limits.deflection().millimetres(),
            step: limits.step(bend),
            floor: limits.floor().millimetres(),
            turn: limits.half_turn_cosine(),
        }
    }

    /// Whether the piece from `a` to `b` is cut at `m`, its middle.
    fn cut(&self, a: &Sample, m: &Sample, b: &Sample) -> bool {
        let reach = (m.point - a.point).norm() + (b.point - m.point).norm();
        reach > 2.0 * self.floor
            && (reach > self.step
                || off_chord(m.point, a.point, b.point) > self.chord
                || self.turns(a, m)
                || self.turns(m, b))
    }

    /// Whether the curve turns through more than the angle allows from `s` to `t`. A derivative of no length turns
    /// nowhere.
    fn turns(&self, s: &Sample, t: &Sample) -> bool {
        s.tangent.dot(t.tangent) < self.turn * s.tangent.norm() * t.tangent.norm()
    }
}

/// How far `p` is from the segment from `a` to `b`: from `a` itself where they are one point.
fn off_chord(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let along = b - a;
    let at = higher(0.0, lower(1.0, (p - a).dot(along) / along.dot(along)));
    (p - (a + along * at)).norm()
}

/// An edge's curve over its range, and whether its ends are one vertex.
struct Piece<'a> {
    curve: &'a dyn Curve,
    range: (f64, f64),
    closed: bool,
}

impl Piece<'_> {
    fn sample(&self, at: f64) -> Sample {
        let (point, tangent) = self.curve.first(at);
        Sample { at, point: point.to_vec3(), tangent: tangent.rate() }
    }

    /// Where the first cuts fall: the ends, four pieces for a closed edge, and pieces of at most a quarter turn where
    /// the parameter is an angle.
    fn first_cuts(&self) -> Vec<f64> {
        let (lo, hi) = self.range;
        let angle = self.curve.range().kind == ParamKind::Angle;
        let least = if self.closed { 4 } else { 1 };
        // The first count of these that is enough, or the last: a list, so that no count runs on, whatever it is asked.
        let enough = |&n: &u32| n >= least && (!angle || hi - lo <= f64::from(n) * FRAC_PI_2);
        let pieces = [1, 2, 4, 8].into_iter().find(enough).unwrap_or(8);
        let at = |k: u32| if k == pieces { hi } else { lo + (hi - lo) * (f64::from(k) / f64::from(pieces)) };
        (0..=pieces).map(at).collect()
    }

    /// The samples `criteria` ask for, depth first from the start; `None` once they would be more than the cap.
    fn cut(&self, criteria: &Criteria) -> Option<Vec<Sample>> {
        let cap = usize::try_from(Limits::SAMPLES_PER_EDGE).unwrap_or(usize::MAX);
        let first: Vec<Sample> = self.first_cuts().into_iter().map(|at| self.sample(at)).collect();
        let mut pending: Vec<(Sample, Sample)> =
            first.iter().zip(first.iter().skip(1)).rev().map(|(&a, &b)| (a, b)).collect();
        let mut done: Vec<Sample> = first.first().copied().into_iter().collect();
        while let Some((a, b)) = pending.pop() {
            let middle = 0.5 * a.at + 0.5 * b.at;
            let m = self.sample(middle);
            if a.at < middle && middle < b.at && criteria.cut(&a, &m, &b) {
                // Each piece still to come ends at a sample of its own, and these two halves at two more.
                if done.len().saturating_add(pending.len()).saturating_add(2) > cap {
                    return None;
                }
                pending.push((m, b));
                pending.push((a, m));
            } else {
                done.push(b);
            }
        }
        Some(done)
    }

    /// The first deflection, doubling from `limits`', at which the piece fits under the cap: `None` if none short of
    /// infinity does.
    fn fitting(&self, bend: f64, limits: Limits) -> Option<Length> {
        let mut deflection = limits.deflection();
        loop {
            deflection = Length::from_millimetres(2.0 * deflection.millimetres());
            let coarser = Limits::new(deflection, limits.angle()).ok()?;
            if self.cut(&Criteria::new(coarser, bend)).is_some() {
                return Some(deflection);
            }
        }
    }
}

#[cfg(test)]
mod tests;
