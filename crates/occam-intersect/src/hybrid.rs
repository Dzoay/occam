//! Source: original
//! The hybrid (P5), the general method: subdivision for start points, then the trace, then the fit. It applies to any
//! pair whose subdivided surfaces have finite domains, after the closed forms and the pencil method, and needs no
//! B-spline form, so offsets and mesh-backed surfaces reach it.
//!
//! Each leaf of the subdivision ([`traverse`](crate::traverse)) gives at most one start point: where Newton's method
//! from its middle settles onto both surfaces inside it, a hair past its edges included, so that a point on an edge is
//! found from both sides. Before that, the gap between the surfaces at its middle and corners is decided against the
//! band, and a leaf where every gap is within it is an overlap the method cannot resolve into curves, as where a
//! rational surface lies on its analytic twin: it gives no start point, and is reported. The leaves are worked through
//! the pair's runner, so their points are made once and in parallel, and they are traced in rounds of [`ROUND`], spread
//! across the leaves so that a round's points are far apart. A point inside the tube about the traces found so far is
//! on a found curve and is not traced; a trace whose stations all lie in the tube of the ones before it found one of
//! them again and is dropped. A trace that is a touch alone is a contact, one that stalled is reported, and the rest are
//! fitted.
//!
//! Every decision goes through occam-math's band decisions and is counted in the section's tally: the subdivision's
//! tests, the leaves' gaps, the tube, and the trace's and fit's own. One the band cannot make, where the answer depends
//! on it, is `decide.undecided`, and nothing is guessed in its place. Lattice seeding alone is not offered: on P5's
//! 625-component surface it returned 256 of 577 curves and said nothing.

use std::sync::Arc;

use occam_geom::{Domain, Surface, UvBox};
use occam_math::{Exec, Grain, Interval, Side};
use occam_report::{Fault, NamedValue, Reason, Report};
use occam_units::{Length, Tolerance, UvPoint, Vec3};

use crate::correct::{Here, Other, Pair, foot};
use crate::decide::{Decide, distance, uncertain, undecided};
use crate::fit::fit;
use crate::methods::{Answer, Method, Spent, SurfacePair, Work, incomplete};
use crate::on::On;
use crate::pieces::{Piece, middle};
use crate::section::{Contact, Section};
use crate::trace::{End, ROUND, Start, Trace, Tracer, Uncertain, Which};
use crate::traverse::{Cell, DEPTH, Held, held, traverse};
use crate::tube::Tube;

/// How far past a leaf's edges, as a part of each side, a point found from it is still its own.
const EDGE: f64 = 1.0 / 1024.0;

/// The general method: subdivision for start points, then the trace, then the fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hybrid {
    /// How the traces step. Its tolerance is replaced by the one the section is asked for.
    pub tracer: Tracer,
    /// The most a leaf of the subdivision is across: each part is halved until its box is at most this.
    pub leaf: Length,
}

impl Method for Hybrid {
    fn name(&self) -> &'static str {
        "hybrid"
    }

    fn attempt(
        &self,
        a: &dyn Surface,
        b: &dyn Surface,
        tol: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        self.run(On::Here(a, b), tol, work, section)
    }

    fn attempt_shared(
        &self,
        exec: &Exec,
        pair: &SurfacePair,
        tol: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        self.run(On::Shared(exec, pair), tol, work, section)
    }
}

/// A start point and where it is.
#[derive(Clone, Copy, Debug)]
struct Seed {
    start: Start,
    at: Vec3,
}

/// What a leaf gives: the gaps between the surfaces at its middle and corners, where they could be measured, where its
/// middle is, and its start point, if any.
struct Found {
    gaps: Option<[Interval; 5]>,
    at: Vec3,
    seed: Option<Seed>,
}

/// What the leaves gave: the start points, in leaf order, and where each overlap is.
#[derive(Default)]
struct Leaves {
    seeds: Vec<Seed>,
    overlaps: Vec<Vec3>,
}

/// What the method met beside what it found: the decisions the band could not make, the unresolved cells and the
/// overlaps, by where each is.
#[derive(Default)]
struct Notes {
    uncertain: Vec<Uncertain>,
    unresolved: Vec<Vec3>,
    overlaps: Vec<Vec3>,
}

impl Hybrid {
    /// The section of `on`'s pair.
    fn run(&self, on: On<'_>, tol: Tolerance, work: &mut Work, section: &mut Section) -> Result<Answer, Spent> {
        let (a, b) = on.surfaces();
        let Some(held) = held(a, b, tol) else { return Ok(Answer::Declined) };
        let tracer = Tracer { tol, ..self.tracer };
        let mut decide = Decide::new(tol.band().millimetres());
        let traversal = traverse(on, (held, self.leaf.millimetres()), work, &mut decide);
        let traversal = match traversal {
            Ok(traversal) => traversal,
            Err(spent) => {
                section.tally = section.tally.merged(decide.tally);
                return Err(spent);
            }
        };
        let mut notes = Notes { uncertain: traversal.uncertain, ..Notes::default() };
        notes.unresolved = traversal.unresolved.iter().filter_map(|cell| where_is(on, cell)).collect();
        let leaves = leaves(on, held, tol, traversal.leaves, &mut decide, &mut notes);
        notes.overlaps = leaves.overlaps;
        let mut found = Vec::new();
        let paid = traced(on, &tracer, &leaves.seeds, work, &mut found, &mut decide, &mut notes);
        let distinct = distinct(found, &tracer, &mut decide, &mut notes, section);
        finish(on, &tracer, &distinct, section);
        section.failures.extend(reports(&notes, tol, section));
        section.tally = section.tally.merged(decide.tally);
        paid.map(|()| Answer::Answered)
    }
}

/// The point at the middle of the first part `cell` holds.
fn where_is(on: On<'_>, cell: &Cell) -> Option<Vec3> {
    let (a, b) = on.surfaces();
    match (&cell.a, &cell.b) {
        (Some(piece), _) => Some(a.point(middle(piece.patch)).to_vec3()),
        (None, Some(piece)) => Some(b.point(middle(piece.patch)).to_vec3()),
        (None, None) => None,
    }
}

/// The middle and the corners of `patch`.
fn samples(patch: UvBox) -> [UvPoint; 5] {
    let [c0, c1, c2, c3] = patch.corners();
    [middle(patch), c0, c1, c2, c3]
}

/// Whether `uv` is inside `patch`, a hair past its edges included, and inside `domain`.
fn inside(patch: UvBox, uv: UvPoint, domain: Domain) -> bool {
    let near = |x: f64, lo: f64, hi: f64| {
        let hair = (hi - lo) * EDGE;
        lo - hair <= x && x <= hi + hair
    };
    near(uv.u(), patch.lo.u(), patch.hi.u()) && near(uv.v(), patch.lo.v(), patch.hi.v()) && domain.contains(uv)
}

/// The start point `pair` settles onto from `seed`, where it is inside `p`'s part and, if given, `q`'s.
fn settled(pair: &Pair<'_>, seed: &Here, p: UvBox, q: Option<UvBox>) -> Option<Seed> {
    let here = pair.settle(seed)?;
    let ours = inside(p, here.p, pair.p.domain()) && q.is_none_or(|q| inside(q, here.q, pair.q.domain()));
    let station = pair.station(&here);
    ours.then_some(Seed { start: Start { on_a: station.on_a, on_b: station.on_b }, at: here.at })
}

/// What the leaf `piece` of `s` gives against `other`, held by `form`.
fn against_form(
    s: &dyn Surface,
    other: &dyn Surface,
    (side, form): (Which, crate::implicit::Implicit),
    piece: &Piece,
    tol: Tolerance,
) -> Found {
    let gaps = samples(piece.patch).map(|uv| form.enclosed(s.point(uv).to_vec3()));
    let pair = Pair { p: s, q: other, other: Other::Implicit(form), swapped: side == Which::B, tol };
    let mid = middle(piece.patch);
    let at = s.point(mid).to_vec3();
    let seed = settled(&pair, &Here { at, p: mid, q: UvPoint::new(0.0, 0.0) }, piece.patch, None);
    Found { gaps: Some(gaps), at, seed }
}

/// What the leaf of parts `p` of `a` and `q` of `b` gives: each gap from `a`'s point to its foot on `b`, found by
/// Gauss-Newton from the middle of `q`, where the foot is in `b`'s domain.
fn against_parts(a: &dyn Surface, b: &dyn Surface, (p, q): (&Piece, &Piece), tol: Tolerance) -> Found {
    let (mid_p, mid_q) = (middle(p.patch), middle(q.patch));
    let gap = |uv: UvPoint| {
        let here = a.point(uv).to_vec3();
        let near = foot(b, here, mid_q);
        b.domain().contains(near).then(|| distance(here, b.point(near).to_vec3()))
    };
    let gaps: Option<Vec<Interval>> = samples(p.patch).into_iter().map(gap).collect();
    let gaps = gaps.and_then(|gaps| <[Interval; 5]>::try_from(gaps).ok());
    let pair = Pair { p: a, q: b, other: Other::Parametric, swapped: false, tol };
    let at = a.point(mid_p).to_vec3();
    let seed = settled(&pair, &Here { at, p: mid_p, q: mid_q }, p.patch, Some(q.patch));
    Found { gaps, at, seed }
}

/// What `leaf` gives, held as `held`.
fn leaf(a: &dyn Surface, b: &dyn Surface, leaf: &Cell, held: Held, tol: Tolerance) -> Option<Found> {
    match (held, &leaf.a, &leaf.b) {
        (Held::One { side: Which::A, form }, Some(piece), _) => Some(against_form(a, b, (Which::A, form), piece, tol)),
        (Held::One { side: Which::B, form }, _, Some(piece)) => Some(against_form(b, a, (Which::B, form), piece, tol)),
        (Held::Both, Some(p), Some(q)) => Some(against_parts(a, b, (p, q), tol)),
        _ => None,
    }
}

/// The start points of `cells`, each a leaf, worked through `on`, and the overlaps among them: a leaf whose every gap
/// is within the band. A leaf whose gaps the band cannot all place, where the rest are within it, keeps its point and
/// gives back the decision.
fn leaves(on: On<'_>, held: Held, tol: Tolerance, cells: Vec<Cell>, decide: &mut Decide, notes: &mut Notes) -> Leaves {
    let found = on.map(Arc::from(cells), Grain::Each, move |a, b, cell| leaf(a, b, cell, held, tol));
    let found: Vec<Found> = found.into_iter().flatten().collect();
    let gaps: Vec<Interval> = found.iter().filter_map(|leaf| leaf.gaps).flatten().collect();
    let mut sides = decide.sides(&gaps).into_iter();
    let mut out = Leaves::default();
    for leaf in found {
        let Some(gaps) = leaf.gaps else {
            out.seeds.extend(leaf.seed);
            continue;
        };
        let mine: Vec<Option<Side>> = sides.by_ref().take(gaps.len()).collect();
        if mine.iter().all(|side| *side == Some(Side::Within)) {
            out.overlaps.push(leaf.at);
            continue;
        }
        if mine.iter().all(|side| matches!(side, Some(Side::Within) | None)) {
            let open = gaps.iter().zip(&mine).find(|(_, side)| side.is_none());
            notes.uncertain.extend(open.map(|(gap, _)| uncertain(*gap, leaf.at)));
        }
        out.seeds.extend(leaf.seed);
    }
    out
}

/// `0..count` in rounds of [`ROUND`], each round taking every so many, so that the points of a round are spread across
/// the leaves.
fn spread(count: usize) -> Vec<usize> {
    let rounds = count.div_ceil(ROUND);
    let round = |r: usize| (0..ROUND).map(move |k| k.saturating_mul(rounds).saturating_add(r));
    (0..rounds).flat_map(round).filter(|&index| index < count).collect()
}

/// The traces from `seeds`, in rounds through `on`, appended to `found`: a seed inside the tube of the traces before
/// its round is not traced.
///
/// # Errors
///
/// [`Spent`] when `work` runs out, `found` holding the traces paid for.
fn traced(
    on: On<'_>,
    tracer: &Tracer,
    seeds: &[Seed],
    work: &mut Work,
    found: &mut Vec<Trace>,
    decide: &mut Decide,
    notes: &mut Notes,
) -> Result<(), Spent> {
    let ordered: Vec<Seed> = spread(seeds.len()).into_iter().filter_map(|index| seeds.get(index).copied()).collect();
    let starts: Vec<Start> = ordered.iter().map(|seed| seed.start).collect();
    let (mut tube, mut next) = (Tube::new(tracer), 0_usize);
    tracer.rounds(on, &starts, work, found, |_, found| {
        for trace in found.get(tube.held..).unwrap_or_default() {
            tube.add(trace);
        }
        let seed = ordered.get(next);
        next = next.saturating_add(1);
        seed.is_some_and(|seed| match tube.holds(seed.at, decide) {
            Ok(inside) => inside,
            Err(open) => {
                notes.uncertain.push(open);
                false
            }
        })
    })
}

/// `found` without the traces that found a curve again: one with no station, or whose first, middle and last stations
/// all lie in the tube of the traces kept before it. Every trace's tally joins the section's.
fn distinct(
    found: Vec<Trace>,
    tracer: &Tracer,
    decide: &mut Decide,
    notes: &mut Notes,
    section: &mut Section,
) -> Vec<Trace> {
    let (mut tube, mut kept) = (Tube::new(tracer), Vec::new());
    for trace in found {
        section.tally = section.tally.merged(trace.tally);
        let probes = [trace.stations.first(), trace.stations.get(trace.stations.len() / 2), trace.stations.last()];
        let again = probes.into_iter().flatten().all(|station| match tube.holds(station.at.to_vec3(), decide) {
            Ok(inside) => inside,
            Err(open) => {
                notes.uncertain.push(open);
                false
            }
        });
        if trace.stations.is_empty() || again {
            continue;
        }
        tube.add(&trace);
        kept.push(trace);
    }
    kept
}

/// The section of the traces `kept`: a touch alone is a contact, the rest that did not stall are fitted through `on`, and each
/// trace's failures follow.
fn finish(on: On<'_>, tracer: &Tracer, kept: &[Trace], section: &mut Section) {
    for trace in kept {
        if let ([station], [End::Touch, End::Touch]) = (trace.stations.as_slice(), trace.ends) {
            section.contacts.push(Contact::Point { at: station.at, on_a: station.on_a, on_b: station.on_b });
        }
    }
    let curves: Arc<[Trace]> =
        kept.iter().filter(|trace| trace.stations.len() > 1 && !trace.ends.contains(&End::Stalled)).cloned().collect();
    let tol = tracer.tol;
    for fitted in on.map(curves, Grain::Each, move |a, b, trace| fit(a, b, trace, tol)) {
        section.curves.extend(fitted.curves);
        section.failures.extend(fitted.failures);
        section.tally = section.tally.merged(fitted.tally);
    }
    let failures: Vec<Report> = kept.iter().flat_map(|trace| tracer.failures(trace, section)).collect();
    section.failures.extend(failures);
}

/// A count of `n` things.
fn count(name: &str, n: usize) -> NamedValue {
    NamedValue::count(name, u32::try_from(n).unwrap_or(u32::MAX))
}

/// `report` with where the first of `places` is, and how many there are.
fn placed(report: Report, places: &[Vec3]) -> Report {
    let report = report.with_value(count("leaves", places.len()));
    match places.first() {
        Some(at) => report.with_value(NamedValue::point("at", [at.x, at.y, at.z])),
        None => report,
    }
}

/// The reports of `notes`, for `found`: `intersect.incomplete` for the overlaps, for `surfaces-coincide`, and for the
/// unresolved cells, for `budget-spent` with how deep a part may be; then `decide.undecided` for each decision the band
/// could not make. None offers a remedy.
fn reports(notes: &Notes, tol: Tolerance, found: &Section) -> Vec<Report> {
    let mut out = Vec::new();
    if !notes.overlaps.is_empty() {
        let report = incomplete(Fault::NotImplemented, Reason::SurfacesCoincide, found);
        out.push(placed(report, &notes.overlaps));
    }
    if !notes.unresolved.is_empty() {
        let report = incomplete(Fault::NumericallySensitive, Reason::BudgetSpent, found);
        out.push(placed(report.with_value(NamedValue::count("depth", DEPTH)), &notes.unresolved));
    }
    let band = tol.band().millimetres();
    out.extend(notes.uncertain.iter().map(|open| undecided(open, band)));
    out
}

/// Where the start points of the leaves `cells` are, in leaf order.
#[cfg(test)]
pub(crate) fn starts(on: On<'_>, held: Held, tol: Tolerance, cells: Vec<Cell>, decide: &mut Decide) -> Vec<Vec3> {
    leaves(on, held, tol, cells, decide, &mut Notes::default()).seeds.iter().map(|seed| seed.at).collect()
}

#[cfg(test)]
mod tests {
    use occam_geom::{ParamRange, Wrap};
    use occam_math::Tally;
    use occam_report::Code;
    use occam_units::{ParamKind, Point};

    use super::*;
    use crate::hybrid_tests::crossing_at_a_corner;
    use crate::trace::Station;
    use crate::trace_tests::{BAND, level, tol, tracer, upright};

    fn patch(u: (f64, f64), v: (f64, f64)) -> UvBox {
        UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1))
    }

    /// A trace through `points`, at their own `x` and `y` as parameters on both surfaces, ending as `ends`.
    fn trace(points: &[(f64, f64, f64)], ends: [End; 2]) -> Trace {
        let station = |&(x, y, z): &(f64, f64, f64)| {
            let uv = UvPoint::new(x, y);
            Station { at: Point::from_vec3(Vec3::new(x, y, z)), on_a: uv, on_b: uv }
        };
        Trace {
            stations: points.iter().map(station).collect(),
            crossings: vec![],
            ends,
            steps: 0,
            tally: Tally::default(),
            undecided: vec![],
        }
    }

    /// `count` points from `from` to `to`, both included.
    fn run(from: (f64, f64), to: (f64, f64), count: u32) -> Vec<(f64, f64, f64)> {
        let at = |k: u32| {
            let f = f64::from(k) / f64::from(count.saturating_sub(1));
            (from.0 + (to.0 - from.0) * f, from.1 + (to.1 - from.1) * f, 0.0)
        };
        (0..count).map(at).collect()
    }

    #[test]
    fn a_point_is_inside_a_part_a_hair_past_its_edges_and_inside_the_domain() {
        let range = ParamRange { lo: 0.0, hi: 10.0, wrap: Wrap::Open, kind: ParamKind::Knot };
        let (part, whole) = (patch((3.0, 4.0), (1.0, 2.0)), Domain { u: range, v: range });
        let hair = 1.0 / 1024.0;
        for (u, v, want) in [
            (3.5, 1.5, true),
            (3.0 + 0.5 * hair, 1.5, true),
            (4.0 - 0.5 * hair, 1.5, true),
            (3.0 - 0.5 * hair, 1.5, true),
            (4.0 + 0.5 * hair, 1.5, true),
            (4.0 + 1.2 * hair, 1.5, false),
            (3.0 - 1.2 * hair, 1.5, false),
            (3.5, 2.0 + 1.2 * hair, false),
            (3.5, 1.0 - 1.2 * hair, false),
            (8.0, 1.5, false),
            (3.5, 8.0, false),
            (8.0, 8.0, false),
        ] {
            assert_eq!(inside(part, UvPoint::new(u, v), whole), want, "{u} {v}");
        }
        let at_the_end = patch((9.0, 10.0), (1.0, 2.0));
        assert!(!inside(at_the_end, UvPoint::new(10.0 + 0.5 * hair, 1.5), whole), "a hair past the domain");
    }

    #[test]
    fn a_start_point_from_two_parts_lies_in_both() {
        // From the middle of the first and a corner of the second, the start settles on the first, far from that corner.
        let (a, b) = crossing_at_a_corner();
        let pair = Pair { p: &a, q: &b, other: Other::Parametric, swapped: false, tol: tol() };
        let (whole, corner) = (patch((-1.0, 1.0), (-1.0, 1.0)), patch((0.9, 1.0), (0.9, 1.0)));
        let seed = Here {
            at: a.point(UvPoint::new(0.0, 0.0)).to_vec3(),
            p: UvPoint::new(0.0, 0.0),
            q: UvPoint::new(0.95, 0.95),
        };
        assert!(settled(&pair, &seed, whole, Some(whole)).is_some());
        assert!(settled(&pair, &seed, whole, Some(corner)).is_none());
    }

    #[test]
    fn start_points_are_taken_in_rounds_of_every_so_many() {
        let order = spread(70);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..70).collect::<Vec<_>>());
        assert_eq!((order.len(), order.get(..4)), (70, Some(&[0, 3, 6, 9][..])));
        assert_eq!(spread(0), Vec::<usize>::new());
    }

    #[test]
    fn a_trace_found_again_is_dropped_by_its_first_middle_and_last_stations() {
        // A bump from 0.3 to 0.7 along the line, 0.3 high: a curve of its own, though it runs on the line either side,
        // its second station included, and leaves it only in the middle.
        let line = trace(&run((0.0, 0.0), (1.0, 0.0), 21), [End::Edge; 2]);
        let mut bump = run((0.0, 0.0), (0.3, 0.0), 7);
        for (from, to, count) in [((0.3, 0.0), (0.5, 0.3), 6), ((0.5, 0.3), (0.7, 0.0), 6), ((0.7, 0.0), (1.0, 0.0), 7)]
        {
            bump.extend(run(from, to, count).into_iter().skip(1));
        }
        let again = trace(&run((0.0, 1e-5), (1.0, 1e-5), 21), [End::Edge; 2]);
        let (mut decide, mut notes, mut section) = (Decide::new(BAND), Notes::default(), Section::default());
        let found = vec![line.clone(), trace(&bump, [End::Edge; 2]), again];
        let kept = distinct(found, &tracer(1e-4, 0.1, 100), &mut decide, &mut notes, &mut section);
        assert_eq!(kept.iter().map(|t| t.stations.len()).collect::<Vec<_>>(), [21, 23]);
        assert_eq!(kept[0], line);
    }

    #[test]
    fn a_stalled_trace_is_reported_not_fitted_and_a_touch_alone_is_a_contact() {
        // The planes z = 0 and x = 0 meet along the y axis.
        let (a, b) = (level(0.0), upright(0.0));
        let mut scratch = occam_geom::Scratch::default();
        let mut on_y = |y: f64| {
            let at = Point::from_vec3(Vec3::new(0.0, y, 0.0));
            let on_b = b.invert(at, None, tol(), &mut scratch).unwrap();
            Station { at, on_a: UvPoint::new(0.0, y), on_b }
        };
        let stations: Vec<Station> = (0..11).map(|k| on_y(0.05 * f64::from(k))).collect();
        let stalled = Trace { stations, ends: [End::Stalled, End::Edge], ..trace(&[], [End::Edge; 2]) };
        let touch = Trace { stations: vec![on_y(2.0)], ends: [End::Touch; 2], ..trace(&[], [End::Edge; 2]) };
        let mut section = Section::default();
        finish(On::Here(&a, &b), &tracer(1e-4, 0.1, 100), &[stalled, touch], &mut section);
        assert_eq!((section.curves.len(), section.contacts.len()), (0, 1));
        assert_eq!(section.failures.iter().map(|r| r.code).collect::<Vec<_>>(), [Code::Incomplete]);
    }
}
