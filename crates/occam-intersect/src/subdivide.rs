//! Source: original
//! A curve against a surface that presents no analytic form: the span and the patch are subdivided together by their
//! bounds, the families' own. A pair of pieces whose boxes do not come within the band of each other is dropped, and the
//! larger of the two is halved until both are small, a sixty-fourth of the first pair's larger box. A side whose box
//! comes back from halving all but a sixteenth as large as before is not halved again: smaller pieces would not be
//! told apart. On each leaf, Gauss-Newton runs from
//! its middle for the nearest pair of points of the curve and the surface, and a pair found inside its leaf, within
//! the band of each other, is a hit: a crossing where the curve meets the surface at more than the angle tolerance, a
//! contact where it is tangent. The ends of the span are tested by the surface's foot of their points, and an end on
//! the surface is a contact. Hits within the band of each other, found from neighbouring leaves, are one, and so are
//! points side by side, one a crossing, whose curve stays within the band between them, as a touch found off its point
//! gives: a contact.
//!
//! Every hit is decided through occam-math's predicates, and one that cannot be is `decide.undecided`. A curve lying
//! in such a surface is not told from many contacts here: its leaves run along it until the budget is spent, which is
//! reported, as a rational surface's coincidence with its analytic twin is reported as unresolved.

use occam_geom::UvBox;
use occam_math::{Interval, Side, Verdict, sin};
use occam_report::Quantity;
use occam_units::{Bounds, UvPoint, Vec3};

use crate::distance::{Spread, judge, shrank};
use crate::hits::{Kind, Sink, Span};
use crate::methods::{Spent, Work};

/// How many times a pair of pieces is split at most.
const DEPTH: u32 = 40;

/// A leaf's boxes are at most this fraction of the first pair's larger box.
const LEAF: f64 = 1.0 / 64.0;

/// How many Gauss-Newton steps a leaf takes at most.
const STEPS: u32 = 32;

/// How far outside its leaf, as a fraction of each side, a point found from it is still its own, so that one on the
/// edge between two leaves is found from both and kept once.
const EDGE: f64 = 1.0 / 1024.0;

/// Which side of a pair of pieces was halved to make it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Halved {
    /// Neither: the first pair.
    Neither,
    /// Its piece of the span.
    Span,
    /// Its piece of the patch.
    Patch,
}

/// A piece of the span against a piece of the patch, with the boxes of the pair it was split from, which side was
/// halved, and which sides no longer shrink when halved.
#[derive(Clone, Copy, Debug)]
struct Cell {
    lo: f64,
    hi: f64,
    patch: UvBox,
    depth: u32,
    parents: Option<(Bounds, Bounds)>,
    halved: Halved,
    stuck: (bool, bool),
}

/// The length of a box's diagonal: NaN or infinity for a box that is not finite.
fn size(bounds: &Bounds) -> f64 {
    (bounds.high().to_vec3() - bounds.low().to_vec3()).norm()
}

/// A point found, with its parameters on the surface.
#[derive(Clone, Copy, Debug)]
struct Found {
    t: f64,
    uv: UvPoint,
    kind: Kind,
}

fn middle(lo: f64, hi: f64) -> f64 {
    lo * 0.5 + hi * 0.5
}

fn centre(patch: UvBox) -> UvPoint {
    UvPoint::new(middle(patch.lo.u(), patch.hi.u()), middle(patch.lo.v(), patch.hi.v()))
}

/// The hits of the sink's curve over `span` on its surface over its patch.
/// A spent budget keeps the points the leaves found before it.
pub(crate) fn subdivide(span: Span, work: &mut Work, sink: &mut Sink<'_>) -> Result<(), Spent> {
    let mut found: Vec<Found> = [span.lo, span.hi].into_iter().filter_map(|t| end(t, sink)).collect();
    let searched = leaves(span, work, sink, &mut found);
    keep(found, sink);
    searched
}

/// Keeps the points found in the order of the curve's parameter, those within the band of each other as one: a
/// contact if any of them is.
fn keep(mut found: Vec<Found>, sink: &mut Sink<'_>) {
    found.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut kept: Vec<Found> = Vec::with_capacity(found.len());
    for next in found {
        match kept.last_mut() {
            Some(last) if same(last, &next, sink) => {
                if next.kind == Kind::Contact {
                    last.kind = Kind::Contact;
                }
            }
            Some(last) if last.kind == Kind::Crossing || next.kind == Kind::Crossing => {
                match touch(last, &next, sink) {
                    Some(between) if last.kind == next.kind => *last = between,
                    Some(_) if last.kind == Kind::Crossing => *last = next,
                    Some(_) => {}
                    None => kept.push(next),
                }
            }
            _ => kept.push(next),
        }
    }
    for hit in kept {
        sink.point(hit.kind, hit.t, Some(hit.uv));
    }
}

/// The leaves, pairs of pieces whose boxes come within the band of each other, small enough or split as far as they
/// go, each solved as it is reached and what it finds added to `found`.
fn leaves(span: Span, work: &mut Work, sink: &mut Sink<'_>, found: &mut Vec<Found>) -> Result<(), Spent> {
    let (curve, surface, tol) = (sink.curve, sink.surface, sink.tol);
    let first = Cell {
        lo: span.lo,
        hi: span.hi,
        patch: sink.patch,
        depth: 0,
        parents: None,
        halved: Halved::Neither,
        stuck: (false, false),
    };
    let (mut stack, mut floor) = (vec![first], None);
    while let Some(cell) = stack.pop() {
        work.spend(1)?;
        let pair = (curve.bounds(cell.lo, cell.hi), surface.bounds(cell.patch));
        if !pair.0.overlaps(&pair.1, tol) {
            continue;
        }
        let settled = match (cell.halved, cell.parents) {
            (Halved::Span, Some((was, _))) => (cell.stuck.0 || !shrank(&was, &pair.0), cell.stuck.1),
            (Halved::Patch, Some((_, was))) => (cell.stuck.0, cell.stuck.1 || !shrank(&was, &pair.1)),
            _ => cell.stuck,
        };
        let cell = Cell { stuck: settled, ..cell };
        let sizes = (size(&pair.0), size(&pair.1));
        let floor = *floor.get_or_insert(if sizes.0 > sizes.1 { sizes.0 } else { sizes.1 } * LEAF);
        let small = (sizes.0 <= floor || settled.0) && (sizes.1 <= floor || settled.1);
        match split(cell, sizes, sink).filter(|_| !small && cell.depth < DEPTH) {
            Some((halves, side)) => {
                for half in halves.into_iter().rev() {
                    let (lo, hi, patch, kept) = (half.lo, half.hi, half.patch, half.stuck);
                    let depth = cell.depth.saturating_add(1);
                    stack.push(Cell { lo, hi, patch, depth, parents: Some(pair), halved: side, stuck: kept });
                }
            }
            None => found.extend(solve(&cell, span, sink)),
        }
    }
    Ok(())
}

/// The two halves of `cell`, and which side they halve: its piece of the span where that box is the larger, otherwise
/// its piece of the patch across its longer side, but never a side that no longer shrinks. `None` where nothing can be
/// halved.
fn split(cell: Cell, sizes: (f64, f64), sink: &Sink<'_>) -> Option<([Cell; 2], Halved)> {
    let (lo, hi) = (cell.patch.lo, cell.patch.hi);
    let mid = centre(cell.patch);
    let (t, u, v) = (middle(cell.lo, cell.hi), mid.u(), mid.v());
    let (span_free, patch_free) = (!cell.stuck.0, !cell.stuck.1);
    let can_t = span_free && cell.lo < t && t < cell.hi;
    let (can_u, can_v) = (patch_free && lo.u() < u && u < hi.u(), patch_free && lo.v() < v && v < hi.v());
    let jet = sink.surface.first(mid);
    let wide_u = jet.du.rate().norm() * (hi.u() - lo.u()) >= jet.dv.rate().norm() * (hi.v() - lo.v());
    let patch = |a: UvPoint, b: UvPoint| Cell { patch: UvBox::new(a, b), ..cell };
    if can_t && (sizes.0 >= sizes.1 || !(can_u || can_v)) {
        Some(([Cell { hi: t, ..cell }, Cell { lo: t, ..cell }], Halved::Span))
    } else if can_u && (wide_u || !can_v) {
        Some(([patch(lo, UvPoint::new(u, hi.v())), patch(UvPoint::new(u, lo.v()), hi)], Halved::Patch))
    } else if can_v {
        Some(([patch(lo, UvPoint::new(hi.u(), v)), patch(UvPoint::new(lo.u(), v), hi)], Halved::Patch))
    } else {
        None
    }
}

/// `x` solving `(a + damping) x = b` for the symmetric 3 × 3 `a`, by Cramer's rule: `None` where it is singular.
fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| {
        let [[m00, m01, m02], [m10, m11, m12], [m20, m21, m22]] = m;
        m00 * (m11 * m22 - m12 * m21) - m01 * (m10 * m22 - m12 * m20) + m02 * (m10 * m21 - m11 * m20)
    };
    let whole = det(a);
    if !(whole.abs() > 0.0 && whole.is_finite()) {
        return None;
    }
    let column = |k: usize| {
        let mut m = a;
        for (row, value) in m.iter_mut().zip(b) {
            if let Some(entry) = row.get_mut(k) {
                *entry = value;
            }
        }
        det(m) / whole
    };
    Some([column(0), column(1), column(2)])
}

/// One Gauss-Newton step from `(t, uv)` towards the nearest pair of points, damped by a millionth of a millionth of the
/// normal matrix's trace so that a tangency, where it is singular, still moves.
fn step(t: f64, uv: UvPoint, sink: &Sink<'_>) -> Option<(f64, UvPoint, f64)> {
    let (point, along) = sink.curve.first(t);
    let jet = sink.surface.first(uv);
    let r = point.to_vec3() - jet.point.to_vec3();
    let zero = Vec3::new(0.0, 0.0, 0.0);
    let columns = [along.rate(), zero - jet.du.rate(), zero - jet.dv.rate()];
    let mut normal = columns.map(|p| columns.map(|q| p.dot(q)));
    let trace: f64 = columns.iter().map(|c| c.dot(*c)).sum();
    for (k, row) in normal.iter_mut().enumerate() {
        if let Some(entry) = row.get_mut(k) {
            *entry += 1e-12 * trace;
        }
    }
    let [dt, du, dv] = solve3(normal, columns.map(|c| -c.dot(r)))?;
    let [ct, cu, cv] = columns;
    let moved = (ct * dt + cu * du + cv * dv).norm();
    Some((t + dt, UvPoint::new(uv.u() + du, uv.v() + dv), moved))
}

/// The nearest pair of points from the middle of `leaf`, if it lies in the leaf and the span, and within the band.
fn solve(leaf: &Cell, span: Span, sink: &mut Sink<'_>) -> Option<Found> {
    let (mut t, mut uv) = (middle(leaf.lo, leaf.hi), centre(leaf.patch));
    let band = sink.tol.band().millimetres();
    for _ in 0..STEPS {
        let (next_t, next_uv, moved) = step(t, uv, sink)?;
        (t, uv) = (next_t, next_uv);
        if moved.is_nan() || moved <= band * 1e-6 {
            break;
        }
    }
    let near = |x: f64, lo: f64, hi: f64| {
        let edge = (hi - lo) * EDGE;
        lo - edge <= x && x <= hi + edge
    };
    let (lo, hi) = (leaf.patch.lo, leaf.patch.hi);
    let inside = near(t, leaf.lo, leaf.hi) && near(uv.u(), lo.u(), hi.u()) && near(uv.v(), lo.v(), hi.v());
    if !(inside && span.lo < t && t < span.hi) {
        return None;
    }
    let uv = sink.in_patch(uv)?;
    if !on(t, uv, sink)? {
        return None;
    }
    Some(Found { t, uv, kind: transversal(t, uv, sink)? })
}

/// Whether the curve's point at `t` and the surface's at `uv` lie within the band of each other: `None`, reported,
/// where that cannot be decided.
fn on(t: f64, uv: UvPoint, sink: &mut Sink<'_>) -> Option<bool> {
    let gap = Spread::at(sink.curve.point(t)).less(Spread::at(sink.surface.point(uv))).length();
    let band = sink.tol.band().millimetres();
    let (verdict, tally) = judge(gap, band);
    sink.tally(tally);
    match verdict {
        Verdict::Decided(side) => Some(side == Side::Within),
        Verdict::Undecided => {
            sink.undecided(t, gap, Quantity::Length, band);
            None
        }
    }
}

/// Whether the curve crosses the surface at `(t, uv)`: the sine of the angle between its tangent and the surface's
/// tangent plane, decided against the sine of the angle tolerance. `None`, reported, where that cannot be decided.
fn transversal(t: f64, uv: UvPoint, sink: &mut Sink<'_>) -> Option<Kind> {
    let (_, along) = sink.curve.first(t);
    let jet = sink.surface.first(uv);
    let normal = jet.du.rate().cross(jet.dv.rate());
    let tangent = along.rate();
    let value = Interval::new(tangent.dot(normal).abs() / (tangent.norm() * normal.norm()), 16.0 * f64::EPSILON);
    let limit = sin(sink.tol.angle().radians());
    let (verdict, tally) = judge(value, limit);
    sink.tally(tally);
    match verdict {
        Verdict::Decided(Side::Within) => Some(Kind::Contact),
        Verdict::Decided(_) => Some(Kind::Crossing),
        Verdict::Undecided => {
            sink.undecided(t, value, Quantity::Ratio, limit);
            None
        }
    }
}

/// An end of the span, `t`, where its point's foot on the surface lies in the patch and within the band of it: a
/// contact.
fn end(t: f64, sink: &mut Sink<'_>) -> Option<Found> {
    let uv = foot(t, None, sink)?;
    on(t, uv, sink)?.then_some(Found { t, uv, kind: Kind::Contact })
}

/// The parameters of the foot on the surface of the curve's point at `t`, searched for from `seed`, if they lie in the
/// patch: `None` where they do not, or where the foot cannot be found, which is reported.
fn foot(t: f64, seed: Option<UvPoint>, sink: &mut Sink<'_>) -> Option<UvPoint> {
    match sink.surface.closest(sink.curve.point(t), seed, sink.tol, &mut sink.scratch) {
        Ok(foot) => sink.in_patch(foot.at),
        Err(report) => {
            sink.fail(report);
            None
        }
    }
}

/// Two points found side by side, one or both a crossing, as one touch, where the curve between them never leaves the
/// band: its point halfway between them in parameter lies within the band of the surface. At this tolerance it did not
/// pass through there: two crossings are one contact, there, and a contact takes in a crossing beside it. `None` where
/// it lies beyond the band, or its foot is outside the patch or cannot be found or decided.
fn touch(a: &Found, b: &Found, sink: &mut Sink<'_>) -> Option<Found> {
    let t = middle(a.t, b.t);
    let uv = foot(t, Some(a.uv), sink)?;
    on(t, uv, sink)?.then_some(Found { t, uv, kind: Kind::Contact })
}

/// Whether two points found are one: within the band of each other.
fn same(a: &Found, b: &Found, sink: &mut Sink<'_>) -> bool {
    let gap = Spread::at(sink.curve.point(a.t)).less(Spread::at(sink.curve.point(b.t))).length();
    let (verdict, tally) = judge(gap, sink.tol.band().millimetres());
    sink.tally(tally);
    verdict == Verdict::Decided(Side::Within)
}

#[cfg(test)]
mod tests {
    use occam_geom::{Curve, Line, OffsetCurve};
    use occam_units::{Direction, Length, Point};

    use super::*;
    use crate::methods::Budget;
    use crate::scan;
    use crate::tests_hits::{found, hit_at, patch, saddle, span, tol, v};
    use crate::trace_tests::level;

    #[test]
    fn a_crossing_beside_a_contact_the_curve_touches_between_is_the_contact() {
        // A line lying in the plane: a crossing at 0.4 and a contact at 0.5 are a touch, kept as the contact.
        let line = Line::new(Point::from_vec3(v(0.0, 0.0, 0.0)), Direction::normalised(v(1.0, 0.0, 0.0)));
        let plane = level(0.0);
        let mut sink = Sink::new(&line, &plane, patch((-1.0, -1.0), (2.0, 1.0)), tol());
        let at = |t: f64, kind| Found { t, uv: UvPoint::new(t, 0.0), kind };
        keep(vec![at(0.4, Kind::Crossing), at(0.5, Kind::Contact)], &mut sink);
        let hits = sink.found();
        assert_eq!((hits.crossings.len(), hits.contacts.len()), (0, 1), "{hits:?}");
        assert_eq!(hits.contacts[0].t, 0.5);
        // Two contacts side by side are two: only a crossing is taken into a touch beside it.
        let mut sink = Sink::new(&line, &plane, patch((-1.0, -1.0), (2.0, 1.0)), tol());
        keep(vec![at(0.4, Kind::Contact), at(0.5, Kind::Contact)], &mut sink);
        assert_eq!(sink.found().contacts.len(), 2);
    }

    #[test]
    fn a_side_is_halved_only_where_it_is_free_and_its_middle_lies_strictly_inside() {
        let line = Line::new(Point::from_vec3(v(0.0, 0.0, 0.0)), Direction::normalised(v(1.0, 0.0, 0.0)));
        let plane = level(0.0);
        let sink = Sink::new(&line, &plane, patch((0.0, 0.0), (2.0, 1.0)), tol());
        let cell = |span: (f64, f64), u: (f64, f64), v: (f64, f64), stuck| Cell {
            lo: span.0,
            hi: span.1,
            patch: patch((u.0, v.0), (u.1, v.1)),
            depth: 0,
            parents: None,
            halved: Halved::Neither,
            stuck,
        };
        let halved = |c: Cell| split(c, (1.0, 1.0), &sink).map(|(_, side)| side);
        let (wide, unit) = ((0.0, 2.0), (0.0, 1.0));
        assert_eq!(halved(cell(unit, wide, unit, (false, false))), Some(Halved::Span));
        assert_eq!(halved(cell(unit, wide, unit, (true, true))), None, "both sides stuck");
        // A range one double wide has its middle at one of its ends, and is not halved.
        for narrow in [(1.0, 1.0_f64.next_up()), (1.0_f64.next_down(), 1.0)] {
            assert_eq!(halved(cell(narrow, wide, unit, (false, false))), Some(Halved::Patch), "{narrow:?}");
            assert_eq!(halved(cell(unit, narrow, narrow, (true, false))), None, "{narrow:?}");
        }
    }

    #[test]
    fn a_span_whose_box_barely_shrinks_is_halved_no_further() {
        // A line 50 to the side of one through a saddle's middle, offset back onto it: its box is 100 across whatever
        // its piece, so the span stops being halved at once, where halving it on would spend far more than this budget.
        let base = Line::new(Point::from_vec3(v(-10.0, 50.0, -0.1)), Direction::normalised(v(1.0, 0.0, 0.01)));
        let up = Direction::normalised(v(0.0, 0.0, 1.0));
        let through = OffsetCurve::new(base.trimmed(0.0, 20.0).unwrap(), Length::from_millimetres(50.0), up).unwrap();
        let on = |t: f64| v(-10.0, 0.0, -0.1) + v(1.0, 0.0, 0.01) * (t / (1.0 + 1e-4_f64).sqrt());
        assert!((through.point(10.0).to_vec3() - on(10.0)).norm() < 1e-12, "{:?}", through.point(10.0));
        let got =
            scan(&through, span(0.0, 20.0), &saddle(), patch((0.0, 0.0), (1.0, 1.0)), tol(), Budget { steps: 2000 });
        found(&got, (1, 0, 0));
        hit_at(&got.crossings[0], 10.0 * (1.0 + 1e-4_f64).sqrt(), &saddle());
    }

    #[test]
    fn a_three_by_three_system_is_solved_and_a_singular_one_refused() {
        let a = [[4.0, 1.0, -2.0], [1.0, 3.0, 0.5], [-2.0, 0.5, 5.0]];
        let b = [1.0, -2.0, 3.0];
        let x = solve3(a, b).unwrap();
        for (row, want) in a.iter().zip(b) {
            let got: f64 = row.iter().zip(x).map(|(m, xi)| m * xi).sum();
            assert!((got - want).abs() < 1e-14, "{x:?}");
        }
        assert_eq!(solve3([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 1.0, 1.0]], b), None, "a row twice another");
        assert_eq!(solve3([[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], b), None, "not finite");
        assert_eq!(solve3([[f64::INFINITY, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], b), None, "not finite");
    }
}
