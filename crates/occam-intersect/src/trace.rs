//! Source: original
//! The trace (P5): from a start point where two surfaces meet, the stations along the curve they meet in, each with its
//! parameters on both, until the curve closes, leaves a surface, ends in a touch, or the trace runs out of steps. It
//! gives points, not curves: `intersection.fit` makes curves of them, and the hybrid's subdivision gives the start
//! points. Its decisions are made against the band and counted in its tally, and one the band cannot make ends it.
//! Traces run in rounds of a fixed size through [`Exec`], merged by index, so their bits do not depend on the number
//! of threads.

use std::sync::Arc;

use occam_geom::{Scratch, Surface};
use occam_math::{Exec, Grain, Tally};
use occam_report::{Fault, NamedValue, Reason, Report};
use occam_units::{Length, Point, Tolerance, UvPoint, Vec3, World};

use crate::correct::{Here, Other, Pair, Side};
use crate::decide::undecided;
use crate::march::{Frame, Limits, Run, frame};
use crate::methods::{Spent, SurfacePair, Work, incomplete};
use crate::on::On;
use crate::project::project;
use crate::section::Section;

/// How many traces run together in a round (P5).
pub const ROUND: usize = 32;

/// Where a trace starts: parameters on each surface near a point where they meet, as a leaf of the hybrid's
/// subdivision gives them. They need not be on the curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    /// The parameters on the first surface.
    pub on_a: UvPoint,
    /// The parameters on the second.
    pub on_b: UvPoint,
}

/// A point where the two surfaces meet, within the band of both, with its parameters on each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// The point.
    pub at: Point<World>,
    /// Its parameters on the first surface.
    pub on_a: UvPoint,
    /// Its parameters on the second.
    pub on_b: UvPoint,
}

/// One of the two surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// The first given.
    A,
    /// The second given.
    B,
}

/// How a surface's parameters jump where the curve carries on across an edge of its domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jump {
    /// A seam, where one end of a parameter meets the other: the curve carries on from the other end.
    Seam,
    /// A pole, a line of the domain that collapses to a point: the curve carries on through it.
    Pole,
}

/// Where a trace crosses a seam or a pole: stations `at` and `at + 1` are the same point, with the parameters on
/// `on` on either side of the edge. The curve is one, and its image on `on` is cut there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    /// The first of the two stations.
    pub at: usize,
    /// The surface whose parameters jump.
    pub on: Which,
    /// What it crosses.
    pub jump: Jump,
}

/// How one end of a trace finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// It came back to its start: the trace is a loop, and its last station leads back to its first.
    Closed,
    /// It reached an edge of a surface's domain that is neither a seam nor a pole: the curve leaves the surface there.
    Edge,
    /// The surfaces touch there: their normals are parallel, where Newton's method loses rank, and alternating
    /// projections converged to the touch. A trace that is a touch alone has one station.
    Touch,
    /// It neither closed nor reached an edge within its steps, or could go no further (L5). It may have jumped to a
    /// neighbouring loop, so its stations are kept but are not known to be one curve.
    Stalled,
    /// No point where both surfaces meet within the band was found near the start, and the trace has no station.
    Apart,
    /// A decision the band cannot make at this resolution stopped it there (O11): whether it closed, whether the
    /// surfaces touch or cross, what an edge is, or whether it moved on. It is in the trace's `undecided`, and nothing
    /// is guessed in its place.
    Undecided,
}

/// A decision the band could not make: the length it was on, its enclosure's radius, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uncertain {
    /// Where the trace was.
    pub at: Point<World>,
    /// The length decided on, the middle of its enclosure.
    pub length: Length,
    /// How far the enclosure reaches either side of it.
    pub radius: Length,
}

/// What a trace found: its stations in order along the curve, from the end it reached marching back from its start to
/// the end it reached marching on.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// The stations, each within the band of both surfaces. A closed trace does not repeat its first at the end.
    pub stations: Vec<Station>,
    /// Where the parameters on a surface jump across a seam or a pole, in station order.
    pub crossings: Vec<Crossing>,
    /// How the first station's end and the last's finished.
    pub ends: [End; 2],
    /// The steps it took: every corrector run and every round of projections.
    pub steps: u32,
    /// The decisions it made against the band (S2).
    pub tally: Tally,
    /// The decisions the band could not make, one for each end that is [`End::Undecided`].
    pub undecided: Vec<Uncertain>,
}

/// How a trace steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tracer {
    /// The tolerance: every station is within its band of both surfaces.
    pub tol: Tolerance,
    /// The most a chord between two stations may sag from the curve: the step is `sqrt(8 sag / curvature)`, so a sag
    /// of the band keeps every chord within the band of the curve.
    pub sag: Length,
    /// The longest step, which also sets the scale below which the trace cannot tell a crossing from a touch.
    pub longest: Length,
    /// The most steps one trace takes before it has stalled. It is checked before each step, so the location of an
    /// edge or a round of projections already begun may take a trace a few steps past it.
    pub steps: u32,
}

impl Tracer {
    /// The trace of where `a` and `b` meet, from `start`. The start is settled onto both surfaces by Newton's method,
    /// or where that loses rank, by alternating projections; a touch there is the trace. Otherwise the trace marches
    /// on from it, and where it does not come back to it, back from it too.
    #[must_use]
    pub fn trace(&self, a: &dyn Surface, b: &dyn Surface, start: Start) -> Trace {
        let pair = Pair::new(a, b, self.tol);
        let mut run = Run::new(&pair, Limits::of(self));
        let first = match begin(&mut run, pair.seed(start)) {
            Ok(Some(Begun::Cross(first))) => first,
            Ok(Some(Begun::Touch(touch))) => {
                return finished(run, vec![pair.station(&touch)], Vec::new(), [End::Touch; 2]);
            }
            Ok(None) => return finished(run, Vec::new(), Vec::new(), [End::Apart; 2]),
            Err((here, uncertain)) => {
                run.undecided.push(uncertain);
                return finished(run, vec![pair.station(&here)], Vec::new(), [End::Undecided; 2]);
            }
        };
        run.stations.push(first.here);
        let ahead = run.march(first, Some(&first));
        let marched = (std::mem::take(&mut run.stations), std::mem::take(&mut run.crossings));
        run.stations.push(first.here);
        let back = if ahead == End::Closed { ahead } else { run.march(Frame { along: -first.along, ..first }, None) };
        let (stations, crossings) =
            joined(&pair, (std::mem::take(&mut run.stations), std::mem::take(&mut run.crossings)), marched);
        finished(run, stations, crossings, [back, ahead])
    }

    /// The traces from each of `starts` on `pair`, appended to `found` in start order: in rounds of [`ROUND`] starts
    /// through `exec`, one trace a task. Before each round, a start for which `skip` holds, given the traces found so
    /// far, is not traced, as one already on a found curve need not be. Each trace's steps are spent from `work` in
    /// start order once its round is done, and a trace whose steps cannot be spent is dropped with the rest.
    ///
    /// # Errors
    ///
    /// [`Spent`] when `work` runs out. `found` keeps every trace paid for before it.
    pub fn traces(
        &self,
        exec: &Exec,
        pair: &SurfacePair,
        starts: &[Start],
        work: &mut Work,
        found: &mut Vec<Trace>,
        skip: impl FnMut(&Start, &[Trace]) -> bool,
    ) -> Result<(), Spent> {
        self.rounds(On::Shared(exec, pair), starts, work, found, skip)
    }

    /// [`Tracer::traces`] on `on`, which runs each round's traces on the calling thread or through `Exec`, with the
    /// same bits.
    pub(crate) fn rounds(
        &self,
        on: On<'_>,
        starts: &[Start],
        work: &mut Work,
        found: &mut Vec<Trace>,
        mut skip: impl FnMut(&Start, &[Trace]) -> bool,
    ) -> Result<(), Spent> {
        for round in starts.chunks(ROUND) {
            let kept: Arc<[Start]> = round.iter().filter(|start| !skip(start, found)).copied().collect();
            let steps = *self;
            for trace in on.map(kept, Grain::Each, move |a, b, start| steps.trace(a, b, *start)) {
                work.spend(trace.steps)?;
                found.push(trace);
            }
        }
        Ok(())
    }

    /// The reports of `trace`'s failures, for the section `found` it belongs to. A trace that stalled gives
    /// `intersect.incomplete` for `budget-spent`, with the trace's steps as its budget, how many stations it kept, and
    /// its first and last. Each decision the band could not make gives `decide.undecided`, with the length decided
    /// on, its radius, the band, where, and the tolerance in force. Neither offers a remedy.
    #[must_use]
    pub fn failures(&self, trace: &Trace, found: &Section) -> Vec<Report> {
        let band = self.tol.band().millimetres();
        let undecided = trace.undecided.iter().map(|uncertain| undecided(uncertain, band));
        let stalled = trace.ends.contains(&End::Stalled).then(|| self.stalled(trace, found));
        stalled.into_iter().chain(undecided).collect()
    }

    /// The report of `trace`, which stalled.
    fn stalled(&self, trace: &Trace, found: &Section) -> Report {
        let points = u32::try_from(trace.stations.len()).unwrap_or(u32::MAX);
        let mut report = incomplete(Fault::NumericallySensitive, Reason::BudgetSpent, found)
            .with_value(NamedValue::count("budget", self.steps))
            .with_value(NamedValue::count("points", points));
        for (name, station) in [("first", trace.stations.first()), ("last", trace.stations.last())] {
            if let Some(station) = station {
                let v = station.at.to_vec3();
                report = report.with_value(NamedValue::point(name, [v.x, v.y, v.z]));
            }
        }
        report
    }
}

/// How a trace's start settled.
enum Begun {
    /// Onto a point where the surfaces cross, from which the trace marches.
    Cross(Frame),
    /// Onto a touch, which is the trace.
    Touch(Here),
}

/// `here` with the parameters of the side held implicitly found again from its point, so that they do not depend on
/// the guess at them that projection started from: the start's, which may be on a cone's other nappe.
fn found_again(pair: &Pair<'_>, here: Here) -> Option<Here> {
    let Other::Implicit(_) = pair.other else { return Some(here) };
    let point = Point::from_vec3(here.at);
    let q = pair.q.closest(point, Some(here.q), pair.tol, &mut Scratch::default()).ok()?.at;
    ((pair.q.point(q).to_vec3() - here.at).norm() <= pair.band()).then_some(Here { q, ..here })
}

/// What `run` found: its stations, crossings and ends, with its steps, tally and undecided decisions.
fn finished(run: Run<'_, '_>, stations: Vec<Station>, crossings: Vec<Crossing>, ends: [End; 2]) -> Trace {
    Trace { stations, crossings, ends, steps: run.steps, tally: run.decide.tally, undecided: run.undecided }
}

/// Where `seed` settles onto both surfaces, inside both domains: by Newton's method where it lands within the
/// longest step of the seed's point on the side it solves on and the surfaces cross there, else by alternating
/// projections from where it landed, or from the seed. Whether they cross or touch is decided against the band.
///
/// # Errors
///
/// A decision the band cannot make, and the station it was made at.
fn begin(run: &mut Run<'_, '_>, seed: Here) -> Result<Option<Begun>, (Here, Uncertain)> {
    let (pair, longest) = (run.pair, run.limits.longest);
    let none = Vec3::new(0.0, 0.0, 0.0);
    run.count(1);
    let near = pair.p.point(seed.p).to_vec3();
    let settled = match pair.settle(&seed).filter(|h| (h.at - near).norm() <= longest) {
        Some(h) => run.inside(h).map_err(|uncertain| (h, uncertain))?,
        None => None,
    };
    if let Some(crossing) = settled.and_then(|h| frame(pair, h, none))
        && !run.touches(&crossing).map_err(|uncertain| (crossing.here, uncertain))?
    {
        return Ok(Some(Begun::Cross(crossing)));
    }
    let from = settled.unwrap_or(seed);
    let (found, rounds) = project(pair, &mut run.decide, &from);
    run.count(rounds);
    let found = found.map_err(|uncertain| (from, uncertain))?;
    let Some(found) = found.and_then(|found| found_again(pair, found)) else { return Ok(None) };
    let Some(found) = run.inside(found).map_err(|uncertain| (found, uncertain))? else { return Ok(None) };
    let Some(at) = frame(pair, found, none) else { return Ok(None) };
    let touches = run.touches(&at).map_err(|uncertain| (found, uncertain))?;
    Ok(Some(if touches { Begun::Touch(found) } else { Begun::Cross(at) }))
}

/// The stations and crossings of a trace from those marched back from its start and on from it, each list beginning
/// with the start: the first reversed, then the second after the start.
fn joined(
    pair: &Pair<'_>,
    (back, back_crossings): (Vec<Here>, Vec<(usize, Side, Jump)>),
    (ahead, ahead_crossings): (Vec<Here>, Vec<(usize, Side, Jump)>),
) -> (Vec<Station>, Vec<Crossing>) {
    let last = back.len().saturating_sub(1);
    let stations = back.iter().rev().chain(ahead.iter().skip(1)).map(|here| pair.station(here)).collect();
    let crossing = |at: usize, side: Side, jump: Jump| Crossing { at, on: pair.which(side), jump };
    let reversed = back_crossings
        .iter()
        .rev()
        .map(|&(at, side, jump)| crossing(last.saturating_sub(at).saturating_sub(1), side, jump));
    let on = ahead_crossings.iter().map(|&(at, side, jump)| crossing(last.saturating_add(at), side, jump));
    (stations, reversed.chain(on).collect())
}
