//! Source: original
//! A loop in its face's parameters. Each use gives a piece: the points of its parameter curve as they are, where it
//! has one, or else its edge's points found on the face's surface and carried round, where a parameter comes round, to
//! follow on from their neighbours. Found points start in the period that the face's sheet names, from a point clear of
//! its seams, so a point within the band of a seam belongs to that period (the determinism topic).
//!
//! Where a point lies against the loop is decided on the pieces' samples, each step refined near the point until how far
//! its arc can stray from its chord settles whether the loop comes within the band of the point. Distances are measured
//! with the surface's rates at the point, so near it they are lengths. How far an arc strays is measured at its middle
//! and not proved, as for a discretisation by deflection (`geometry.measures`).

use occam_geom::{Curve, ParamCurve, ParamRange, Scratch, Surface, Wrap};
use occam_report::{EntityRef, Quantity};
use occam_units::{Length, ParamKind, Tolerance, UvPoint};

use super::{winding, within};
use crate::kind::{EdgeUse, Id, Loop};
use crate::snapshot::Model;
use crate::view::{View, view};

/// How many steps each use's piece is first cut into.
const STEPS: u32 = 16;

/// How many times a step is halved at most.
const DEEPEST: u32 = 24;

/// How many halvings deciding one point against a loop may take in all, so that no loop, however it is drawn, can make
/// the decision draw without end: past them, how far the loop keeps from the point cannot be measured.
const HALVINGS: u32 = 1 << 16;

/// Where a use's piece of the loop comes from.
#[derive(Clone, Copy)]
enum Source<'a> {
    /// The use's parameter curve, whose points are in the face's parameters as they are.
    Given(&'a dyn ParamCurve),
    /// The edge's curve, whose points are found on the surface.
    Found(&'a dyn Curve),
}

/// One use's piece of the loop: its edge's parameter from `from` to `to`, in the use's direction.
#[derive(Clone, Copy)]
struct Piece<'a> {
    of: EntityRef,
    source: Source<'a>,
    from: f64,
    to: f64,
}

/// A point of the loop: which piece, the edge's parameter there, and the point in the face's parameters.
#[derive(Clone, Copy)]
struct Sample {
    piece: usize,
    t: f64,
    uv: UvPoint,
}

/// The values one parameter takes in the period the face's sheet names, or over its whole range where it does not
/// come round. A parameter that does not come round has no other period, so a sheet that names one has none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Chart {
    pub(super) lo: f64,
    pub(super) hi: f64,
    pub(super) sheet: i16,
    pub(super) quantity: Quantity,
    period: Option<f64>,
    kind: ParamKind,
    exists: bool,
}

impl Chart {
    /// The chart of a parameter with `range`, for a face whose sheet names its period `sheet`.
    fn of(range: ParamRange, sheet: i16) -> Self {
        let quantity = range.quantity();
        let base = Self { lo: range.lo, hi: range.hi, sheet, quantity, period: None, kind: range.kind, exists: true };
        if range.wrap == Wrap::Periodic {
            let period = range.hi - range.lo;
            let shift = f64::from(sheet) * period;
            Self { lo: range.lo + shift, hi: range.hi + shift, period: Some(period), ..base }
        } else {
            Self { exists: sheet == 0, ..base }
        }
    }

    /// How far `x` lies beyond the chart, below zero inside it.
    fn beyond(self, x: f64) -> f64 {
        let (below, above) = (self.lo - x, x - self.hi);
        if !self.exists {
            f64::INFINITY
        } else if below > above {
            below
        } else {
            above
        }
    }

    /// The value equal to `x` nearest `near`, for a parameter that comes round.
    fn nearest(self, x: f64, near: f64) -> f64 {
        self.period.map_or(x, |p| x + p * ((near - x) / p).round())
    }

    /// The value equal to `x` in the chart, for a parameter that comes round.
    fn placed(self, x: f64) -> f64 {
        self.period.map_or(x, |p| self.lo + (x - self.lo).rem_euclid(p))
    }
}

/// Where the loop does not close: the uses on either side, and how far apart they are in each parameter.
pub(super) struct Joint {
    pub(super) after: EntityRef,
    pub(super) before: EntityRef,
    pub(super) gap: [f64; 2],
}

/// The point of the loop furthest outside its chart in one parameter: which one, its value, and the chart.
pub(super) struct Off {
    pub(super) param: usize,
    pub(super) value: f64,
    pub(super) chart: Chart,
}

/// Where a point lies against a loop: its winding number, and how far the loop keeps from it at least, measured as a
/// length near the point; or undecided, where neither can be had.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Place {
    pub(super) winding: Option<i32>,
    pub(super) clearance: f64,
}

impl Place {
    /// Inside, by more than `band`.
    pub(super) fn inside(self, band: f64) -> bool {
        self.winding.is_some_and(|w| w != 0) && self.clearance > band
    }

    /// Outside, by more than `band`.
    pub(super) fn outside(self, band: f64) -> bool {
        self.winding == Some(0) && self.clearance > band
    }
}

/// A loop in its face's parameters.
pub(super) struct Trace<'a> {
    surface: &'a dyn Surface,
    tol: Tolerance,
    pieces: Vec<Piece<'a>>,
    samples: Vec<Sample>,
    pub(super) charts: [Chart; 2],
}

/// The value a fraction `k / STEPS` of the way from `from` to `to`, `to` itself at the end.
fn along(from: f64, to: f64, k: u32) -> f64 {
    if k >= STEPS { to } else { from + (to - from) * (f64::from(k) / f64::from(STEPS)) }
}

/// The piece of the use `id`, if every record it reads is there.
fn piece(model: &Model, id: Id<EdgeUse>) -> Option<Piece<'_>> {
    let u = view(model, id)?;
    let e = view(model, u.edge())?;
    let (lo, hi) = e.range();
    let (from, to) = if u.reversed() { (hi, lo) } else { (lo, hi) };
    let source = match u.pcurve() {
        Some(pcurve) => Source::Given(pcurve),
        None => Source::Found(e.curve()?),
    };
    Some(Piece { of: id.entity(), source, from, to })
}

impl<'a> Trace<'a> {
    /// The loop `l` in its face's parameters, if every record it reads is there and every point is found.
    pub(super) fn of(model: &'a Model, l: View<'a, Loop>) -> Option<Self> {
        let face = view(model, l.face())?;
        let surface = face.surface()?;
        let pieces: Vec<Piece<'a>> = l.uses().into_iter().map(|u| piece(model, u)).collect::<Option<_>>()?;
        let (domain, sheet) = (surface.domain(), face.sheet());
        let charts = [Chart::of(domain.u, sheet.u), Chart::of(domain.v, sheet.v)];
        let mut trace = Self { surface, tol: model.tolerance, pieces, samples: Vec::new(), charts };
        trace.sample()?;
        trace.carry();
        (!trace.samples.is_empty()).then_some(trace)
    }

    /// Whether piece `index` is given in the face's parameters.
    fn given(&self, index: usize) -> bool {
        self.pieces.get(index).is_some_and(|p| matches!(p.source, Source::Given(_)))
    }

    /// The point of `source` at `t`: as given, or found on the surface from `seed` and left where it is found.
    fn point(&self, source: Source<'_>, t: f64, seed: Option<UvPoint>, scratch: &mut Scratch) -> Option<UvPoint> {
        match source {
            Source::Given(pcurve) => Some(pcurve.uv(t)),
            Source::Found(curve) => self.surface.invert(curve.point(t), seed, self.tol, scratch).ok(),
        }
    }

    /// Samples every piece at `STEPS` even steps, its ends included.
    fn sample(&mut self) -> Option<()> {
        let (mut scratch, mut seed) = (Scratch::default(), None);
        let mut samples = Vec::new();
        for (index, piece) in self.pieces.iter().enumerate() {
            for k in 0..=STEPS {
                let t = along(piece.from, piece.to, k);
                let uv = self.point(piece.source, t, seed, &mut scratch)?;
                seed = Some(uv);
                samples.push(Sample { piece: index, t, uv });
            }
        }
        self.samples = samples;
        Some(())
    }

    /// The band in each parameter at `uv`: the change in it that moves the surface's point by the length band.
    fn bands(&self, uv: UvPoint) -> [f64; 2] {
        let jet = self.surface.first(uv);
        let rate = |r: f64, chart: Chart| self.tol.in_parameter(chart.kind, Length::from_millimetres(r));
        [rate(jet.du.rate().norm(), self.charts[0]), rate(jet.dv.rate().norm(), self.charts[1])]
    }

    /// `uv` with each parameter that comes round placed in its chart.
    fn placed(&self, uv: UvPoint) -> UvPoint {
        UvPoint::new(self.charts[0].placed(uv.u()), self.charts[1].placed(uv.v()))
    }

    /// `uv` with each parameter that comes round moved by whole periods to lie nearest `near`.
    fn nearest(charts: [Chart; 2], uv: UvPoint, near: UvPoint) -> UvPoint {
        UvPoint::new(charts[0].nearest(uv.u(), near.u()), charts[1].nearest(uv.v(), near.v()))
    }

    /// Whether `uv`, placed in the charts, is clear of every seam by more than the band.
    fn clear_of_seams(&self, uv: UvPoint) -> bool {
        let placed = self.placed(uv);
        let bands = self.bands(placed);
        let clear = |chart: Chart, x: f64, band: f64| chart.period.is_none() || -chart.beyond(x) > band;
        clear(self.charts[0], placed.u(), bands[0]) && clear(self.charts[1], placed.v(), bands[1])
    }

    /// Carries every found point round to follow on from its neighbour, from a first one that is given, or else is
    /// clear of the seams and placed in the charts.
    fn carry(&mut self) {
        let anchor = (0..self.samples.len()).find(|&k| self.samples.get(k).is_some_and(|s| self.given(s.piece)));
        let clear = || self.samples.iter().position(|s| self.clear_of_seams(s.uv));
        let anchor = anchor.or_else(clear).unwrap_or(0);
        if let Some(s) = self.samples.get(anchor).copied().filter(|s| !self.given(s.piece)) {
            let placed = self.placed(s.uv);
            if let Some(first) = self.samples.get_mut(anchor) {
                first.uv = placed;
            }
        }
        let forward = (anchor.saturating_add(1)..self.samples.len()).map(|k| (k, k.checked_sub(1)));
        let backward = (0..anchor).rev().map(|k| (k, k.checked_add(1)));
        for (k, from) in forward.chain(backward) {
            let near = from.and_then(|j| self.samples.get(j)).map(|s| s.uv);
            let found = self.samples.get(k).is_some_and(|s| !self.given(s.piece));
            if let (Some(near), true, Some(s)) = (near, found, self.samples.get_mut(k)) {
                s.uv = Self::nearest(self.charts, s.uv, near);
            }
        }
    }
}

/// Distances near a point of the face's parameters, each parameter scaled by the surface's rate there, so that near
/// the point they are lengths.
#[derive(Clone, Copy)]
struct Metric {
    q: UvPoint,
    scale: [f64; 2],
}

impl Metric {
    /// `p` relative to the point, scaled.
    fn at(self, p: UvPoint) -> [f64; 2] {
        [(p.u() - self.q.u()) * self.scale[0], (p.v() - self.q.v()) * self.scale[1]]
    }

    /// How far `p` is from the segment `a` to `b`: from the nearer end where its foot on the line is beyond one, else
    /// from the line.
    fn distance(self, p: UvPoint, a: UvPoint, b: UvPoint) -> f64 {
        let ([pu, pv], [au, av], [bu, bv]) = (self.at(p), self.at(a), self.at(b));
        let (du, dv, wu, wv) = (bu - au, bv - av, pu - au, pv - av);
        let (squared, along) = (du * du + dv * dv, wu * du + wv * dv);
        if along <= 0.0 {
            (wu * wu + wv * wv).sqrt()
        } else if along >= squared {
            let (eu, ev) = (pu - bu, pv - bv);
            (eu * eu + ev * ev).sqrt()
        } else {
            (wu * dv - wv * du).abs() / squared.sqrt()
        }
    }
}

/// The segments a loop is drawn with near a point, and the least clearance they leave it.
type Segments = (Vec<[UvPoint; 2]>, f64);

/// The least of the clearances so far and `here`, where a clearance that cannot be measured stays so.
fn least(so_far: f64, here: f64) -> f64 {
    if so_far.is_nan() {
        so_far
    } else if here.is_nan() || here < so_far {
        here
    } else {
        so_far
    }
}

impl Trace<'_> {
    /// The samples of piece `index`, in order.
    fn of_piece(&self, index: usize) -> impl Iterator<Item = &Sample> {
        self.samples.iter().filter(move |s| s.piece == index)
    }

    /// The first joint, in the loop's order, where one piece's end and the next piece's start are further apart in
    /// either parameter than the band there.
    pub(super) fn open(&self) -> Option<Joint> {
        let count = self.pieces.len();
        (0..count).find_map(|i| {
            let j = i.checked_add(1).filter(|&j| j < count).unwrap_or(0);
            let (end, start) = (self.of_piece(i).last()?, self.of_piece(j).next()?);
            let gap = [(start.uv.u() - end.uv.u()).abs(), (start.uv.v() - end.uv.v()).abs()];
            let bands = self.bands(end.uv);
            if within(gap[0], bands[0]) && within(gap[1], bands[1]) {
                return None;
            }
            Some(Joint { after: self.pieces.get(i)?.of, before: self.pieces.get(j)?.of, gap })
        })
    }

    /// The point furthest outside its chart by more than the band there, in either parameter, if one is.
    pub(super) fn off_sheet(&self) -> Option<Off> {
        let mut worst: Option<(f64, Off)> = None;
        for s in &self.samples {
            let bands = self.bands(s.uv);
            for (param, (chart, x)) in self.charts.into_iter().zip([s.uv.u(), s.uv.v()]).enumerate() {
                let band = bands.get(param).copied().unwrap_or(f64::NAN);
                let excess = chart.beyond(x);
                if within(excess, band) {
                    continue;
                }
                let by = excess - band;
                if worst.as_ref().is_none_or(|(most, _)| by > *most) {
                    worst = Some((by, Off { param, value: x, chart }));
                }
            }
        }
        worst.map(|(_, off)| off)
    }

    /// The point of piece `a.piece` halfway between `a` and `b` in the edge's parameter, carried to follow on from `a`.
    fn between(&self, a: Sample, b: Sample) -> Option<Sample> {
        let source = self.pieces.get(a.piece)?.source;
        let t = a.t.midpoint(b.t);
        let uv = self.point(source, t, Some(a.uv), &mut Scratch::default())?;
        let uv = if self.given(a.piece) { uv } else { Self::nearest(self.charts, uv, a.uv) };
        Some(Sample { piece: a.piece, t, uv })
    }

    /// The step from `a` to `b` as segments, halved while how far its arc may stray from its chord leaves it unsure
    /// whether the loop comes within `band` of the metric's point, each with the least clearance it leaves, spending
    /// one of the halvings `left` on each. A joint between pieces, or round from the last sample to the first, is a
    /// straight segment. A step still unsure when none are left leaves a clearance that cannot be measured.
    fn refine(&self, (a, b, joint): (Sample, Sample, bool), metric: Metric, band: f64, out: (&mut Segments, &mut u32)) {
        let (out, left) = out;
        let mut stack = vec![(a, b, 0_u32)];
        while let Some((a, b, depth)) = stack.pop() {
            let mid = if joint { None } else { self.between(a, b) };
            let chord = metric.distance(metric.q, a.uv, b.uv);
            let sag = mid.map_or(0.0, |m| metric.distance(m.uv, a.uv, b.uv));
            let unsure = chord - sag <= band && band < chord + sag;
            if let (true, Some(m)) = (unsure && depth < DEEPEST, mid) {
                if *left == 0 {
                    out.1 = f64::NAN;
                } else {
                    *left = left.saturating_sub(1);
                    stack.push((m, b, depth.saturating_add(1)));
                    stack.push((a, m, depth.saturating_add(1)));
                    continue;
                }
            }
            out.1 = least(out.1, chord - sag);
            out.0.push([a.uv, b.uv]);
        }
    }

    /// Where `q` lies against the loop, deciding to `band`.
    pub(super) fn place(&self, q: UvPoint, band: f64) -> Place {
        self.place_within(q, band, HALVINGS)
    }

    /// As [`Trace::place`], with `halvings` to spend in all.
    fn place_within(&self, q: UvPoint, band: f64, mut halvings: u32) -> Place {
        let jet = self.surface.first(q);
        let scale = [jet.du.rate().norm(), jet.dv.rate().norm()];
        if !scale.iter().all(|s| s.is_finite()) {
            return Place { winding: None, clearance: f64::NAN };
        }
        let metric = Metric { q, scale };
        let mut out = (Vec::new(), f64::INFINITY);
        let closing = self.samples.last().copied().zip(self.samples.first().copied()).map(|(a, b)| (a, b, true));
        let steps = self.samples.windows(2).filter_map(|pair| match pair {
            [a, b] => Some((*a, *b, a.piece != b.piece)),
            _ => None,
        });
        for step in steps.chain(closing) {
            self.refine(step, metric, band, (&mut out, &mut halvings));
        }
        Place { winding: winding::number(&out.0, q), clearance: out.1 }
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use super::*;

    fn uv(u: f64, v: f64) -> UvPoint {
        UvPoint::new(u, v)
    }

    fn turn(sheet: i16) -> Chart {
        Chart::of(ParamRange { lo: 0.0, hi: TAU, wrap: Wrap::Periodic, kind: ParamKind::Angle }, sheet)
    }

    #[test]
    fn a_chart_is_the_period_its_sheet_names_or_the_whole_range() {
        assert_eq!(
            [turn(0), turn(1), turn(-2)].map(|c| (c.lo, c.hi)),
            [(0.0, TAU), (TAU, 2.0 * TAU), (-2.0 * TAU, -TAU)]
        );
        assert_eq!((turn(1).sheet, turn(1).quantity), (1, Quantity::Angle));
        let line = ParamRange::unbounded(ParamKind::Length);
        assert!(Chart::of(line, 0).beyond(1e300) < 0.0, "a line has every value");
        assert_eq!(Chart::of(line, 1).beyond(0.0), f64::INFINITY, "and no other period");
        let part = Chart::of(ParamRange { lo: 1.0, hi: 3.0, wrap: Wrap::Closed, kind: ParamKind::Length }, 0);
        assert_eq!([part.beyond(0.5), part.beyond(3.25), part.beyond(2.0), part.beyond(1.5)], [0.5, 0.25, -1.0, -0.5]);
        assert!(part.beyond(f64::NAN).is_nan());
    }

    #[test]
    fn a_value_is_carried_round_to_the_nearest_of_its_equals_or_placed_in_its_chart() {
        assert_eq!(turn(0).nearest(0.25, 6.0), 0.25 + TAU);
        assert_eq!(turn(0).nearest(6.0, 0.25), 6.0 - TAU);
        assert_eq!(turn(0).nearest(3.0, 3.5), 3.0);
        assert_eq!(turn(0).placed(-0.5), (-0.5_f64).rem_euclid(TAU));
        assert_eq!(turn(1).placed(0.25), TAU + 0.25);
        assert_eq!(turn(-1).placed(0.25), -TAU + 0.25);
        let line = Chart::of(ParamRange::unbounded(ParamKind::Length), 0);
        assert_eq!((line.nearest(5.0, 100.0), line.placed(-7.0)), (5.0, -7.0));
    }

    #[test]
    fn the_distance_from_a_segment_is_from_its_nearer_end_beyond_it_and_else_from_its_line() {
        let plain = Metric { q: uv(0.0, 0.0), scale: [1.0, 1.0] };
        let (a, b) = (uv(0.0, 0.0), uv(2.0, 0.0));
        assert_eq!(plain.distance(uv(1.0, 1.0), a, b), 1.0);
        assert_eq!(plain.distance(uv(1.0, -0.5), a, b), 0.5);
        assert_eq!(plain.distance(uv(-3.0, 4.0), a, b), 5.0, "beyond its start");
        assert_eq!(plain.distance(uv(5.0, 4.0), a, b), 5.0, "beyond its finish");
        assert_eq!(plain.distance(uv(3.0, 4.0), a, a), 5.0, "a segment of no length");
        let stretched = Metric { q: uv(1.0, 0.0), scale: [2.0, 3.0] };
        assert_eq!(stretched.at(uv(2.0, 1.0)), [2.0, 3.0]);
        assert_eq!(stretched.distance(uv(1.0, 1.0), a, b), 3.0);
        assert_eq!(stretched.distance(uv(-1.0, 0.0), a, b), 2.0);
    }

    #[test]
    fn the_least_clearance_keeps_one_that_cannot_be_measured() {
        assert_eq!((least(1.0, 2.0), least(2.0, 1.0), least(f64::INFINITY, 3.0)), (1.0, 1.0, 3.0));
        assert!(least(f64::NAN, 0.5).is_nan() && least(0.5, f64::NAN).is_nan());
    }

    #[test]
    fn the_steps_reach_the_end_exactly() {
        assert_eq!(
            [along(0.1, 0.7, STEPS), along(0.0, 1.0, 0), along(0.0, 1.0, 8), along(1.0, 0.0, 4)],
            [0.7, 0.0, 0.5, 0.75]
        );
    }

    #[test]
    fn a_place_is_inside_or_outside_only_by_more_than_the_band() {
        let place = |winding, clearance| Place { winding, clearance };
        assert!(
            place(Some(1), 2.0).inside(1.0) && place(Some(-1), 2.0).inside(1.0) && !place(Some(1), 2.0).outside(1.0)
        );
        assert!(place(Some(0), 2.0).outside(1.0) && !place(Some(0), 2.0).inside(1.0));
        assert!(!place(Some(1), 1.0).inside(1.0) && !place(Some(0), 1.0).outside(1.0), "at the band itself");
        assert!(!place(None, 2.0).inside(1.0) && !place(None, 2.0).outside(1.0));
        assert!(!place(Some(1), f64::NAN).inside(1.0) && !place(Some(0), f64::NAN).outside(1.0));
    }

    #[test]
    fn the_distance_from_any_segment_is_measured_about_the_point_in_its_metric() {
        // About q = (0.5, -0.25), scaled by 2 and 3: a = (1, 2) and b = (4, 6) are (1, 6.75) and (7, 18.75).
        let m = Metric { q: uv(0.5, -0.25), scale: [2.0, 3.0] };
        assert_eq!(m.at(uv(1.0, 2.0)), [1.0, 6.75]);
        let (a, b) = (uv(1.0, 2.0), uv(4.0, 6.0));
        let near = |got: f64, want: f64| (got - want).abs() < 1e-12;
        assert!(near(m.distance(uv(3.0, 3.0), a, b), 5f64.sqrt()), "from its line: (4, 3) across (6, 12)");
        assert!(near(m.distance(uv(0.0, 0.0), a, b), 40f64.sqrt()), "beyond its start: (-2, -6)");
        assert!(near(m.distance(uv(6.0, 8.0), a, b), 52f64.sqrt()), "beyond its finish: (4, 6)");
    }

    #[test]
    fn a_chart_of_a_turn_that_starts_anywhere_is_as_long_as_the_turn() {
        let from_one = |sheet| {
            Chart::of(ParamRange { lo: 1.0, hi: 1.0 + TAU, wrap: Wrap::Periodic, kind: ParamKind::Angle }, sheet)
        };
        let next = from_one(1);
        assert!((next.lo - (1.0 + TAU)).abs() < 1e-12 && (next.hi - (1.0 + 2.0 * TAU)).abs() < 1e-12, "{next:?}");
        assert!((from_one(0).placed(0.25) - (0.25 + TAU)).abs() < 1e-12, "a turn on, to lie from 1");
    }

    /// A trace of no pieces on `surface`, with `charts`, to which samples can be given.
    fn bare(surface: &dyn Surface, charts: [Chart; 2], samples: Vec<Sample>) -> Trace<'_> {
        Trace { surface, tol: crate::testing::tolerance(), pieces: Vec::new(), samples, charts }
    }

    fn cylinder() -> occam_geom::Cylinder {
        occam_geom::Cylinder::placed(&occam_units::Placement::identity(), Length::from_millimetres(2.0)).unwrap()
    }

    #[test]
    fn a_point_is_clear_of_the_seams_by_more_than_the_band_there() {
        // A cylinder of radius 2: a turn of u, whose band is the length band over 2, and v without end.
        let c = cylinder();
        let domain = Surface::domain(&c);
        let t = bare(&c, [Chart::of(domain.u, 0), Chart::of(domain.v, 0)], Vec::new());
        let band = t.bands(uv(1.0, 0.0))[0];
        assert!((band - crate::testing::tolerance().band().millimetres() / 2.0).abs() < 1e-24, "{band}");
        assert!(t.clear_of_seams(uv(3.0, 0.5)) && t.clear_of_seams(uv(2.0 * band, 0.5)));
        assert!(!t.clear_of_seams(uv(band, 0.5)), "exactly the band from the seam is not clear of it");
        assert!(!t.clear_of_seams(uv(0.5 * band, 0.5)) && !t.clear_of_seams(uv(TAU - 0.5 * band, 7.0)));
    }

    fn sample(piece: usize, u: f64, v: f64) -> Sample {
        Sample { piece, t: 0.0, uv: uv(u, v) }
    }

    #[test]
    fn a_loop_is_open_where_a_piece_does_not_begin_where_the_one_before_ends_round_to_the_first() {
        let plane = occam_geom::Plane::placed(&occam_units::Placement::identity());
        let line = occam_geom::Line::new(
            occam_units::Point::from_vec3(occam_units::Vec3::new(0.0, 0.0, 0.0)),
            occam_units::Direction::normalised(occam_units::Vec3::new(1.0, 0.0, 0.0)),
        );
        let given = occam_geom::UvCurve::new(line).unwrap();
        let use_of = |slot| EntityRef::new(occam_report::EntityKind::EdgeUse, slot, 1);
        let pieces: Vec<Piece<'_>> =
            (0..3).map(|k| Piece { of: use_of(k), source: Source::Given(&given), from: 0.0, to: 1.0 }).collect();
        let charts = [Chart::of(Surface::domain(&plane).u, 0), Chart::of(Surface::domain(&plane).v, 0)];
        let triangle = |last: (f64, f64)| {
            let samples = vec![
                sample(0, 0.0, 0.0),
                sample(0, 1.0, 0.0),
                sample(1, 1.0, 0.0),
                sample(1, 1.0, 1.0),
                sample(2, 1.0, 1.0),
                sample(2, last.0, last.1),
            ];
            Trace { pieces: pieces.clone(), ..bare(&plane, charts, samples) }
        };
        assert!(triangle((0.0, 0.0)).open().is_none(), "closed");
        let gap = triangle((0.0, 1e-3)).open().unwrap();
        assert_eq!((gap.after, gap.before), (use_of(2), use_of(0)), "round from the last to the first");
        assert_eq!(gap.gap, [0.0, 1e-3], "open in v alone");
    }

    #[test]
    fn the_point_furthest_off_its_sheet_is_by_how_far_beyond_the_band_it_is() {
        // Charts from 0 to 1 in each parameter of a cylinder of radius 2, where u's band is half v's.
        let c = cylinder();
        let unit = Chart::of(ParamRange { lo: 0.0, hi: 1.0, wrap: Wrap::Closed, kind: ParamKind::Length }, 0);
        let off = |a: f64, b: f64| {
            let t = bare(&c, [unit, unit], vec![sample(0, 1.0 + a, 0.5), sample(0, 0.5, 1.0 + b)]);
            t.off_sheet().map(|o| o.param)
        };
        assert_eq!(off(1e-3, 1e-3), Some(0), "as far out in each: u, whose band is less, by more");
        assert_eq!(off(1e-3, 1.5e-3), Some(1), "v further out, by more than its band is wider");
        let plane = occam_geom::Plane::placed(&occam_units::Placement::identity());
        let tied = bare(&plane, [unit, unit], vec![sample(0, 1.002, 0.5), sample(0, 0.5, 1.002)]);
        assert_eq!(tied.off_sheet().map(|o| o.param), Some(0), "of two as far, the first");
    }

    #[test]
    fn a_point_between_a_circle_and_its_chords_is_inside_it_once_the_steps_are_halved() {
        // A unit circle drawn in sixteen steps, and a point at radius 0.99 halfway along the first: outside the chord,
        // 0.0092 beyond it, but inside the circle, 0.01 from it.
        let plane = occam_geom::Plane::placed(&occam_units::Placement::identity());
        let circle =
            occam_geom::Circle::placed(&occam_units::Placement::identity(), Length::from_millimetres(1.0)).unwrap();
        let given = occam_geom::UvCurve::new(circle).unwrap();
        let of = EntityRef::new(occam_report::EntityKind::EdgeUse, 0, 1);
        let charts = [Chart::of(Surface::domain(&plane).u, 0), Chart::of(Surface::domain(&plane).v, 0)];
        let pieces = vec![Piece { of, source: Source::Given(&given), from: 0.0, to: TAU }];
        let mut t = Trace { pieces, ..bare(&plane, charts, Vec::new()) };
        t.sample().unwrap();
        t.carry();
        let (s, c) = occam_math::sin_cos(TAU / 32.0);
        let q = uv(0.99 * c, 0.99 * s);
        let band = crate::testing::tolerance().band().millimetres();
        let settled = t.place(q, band);
        // Inside by more than the band, settled before the clearance, a bound from below, reaches the distance, 0.01.
        assert_eq!(settled.winding.map(i32::abs), Some(1), "{settled:?}");
        assert!(settled.inside(band) && settled.clearance <= 0.01, "{settled:?}");
        // Just off the chord, whose middle is at radius cos(τ/32), 1e-6 beyond it towards the arc: the steps are halved
        // until how far the arc strays settles it.
        let hugging = uv((c + 1e-6) * c, (c + 1e-6) * s);
        assert!(t.place(hugging, band).inside(band), "{:?}", t.place(hugging, band));
        let spent = t.place_within(q, band, 0);
        assert!(spent.clearance.is_nan(), "with no halvings to spend, how far cannot be measured: {spent:?}");
    }
}
