//! Source: rump-1999
//! A curve against a surface that presents an analytic form: the roots of the surface's signed distance along the
//! curve, isolated by interval bounds over subranges (Rump 1999). A piece of the span whose box lies certainly on one
//! side of the band is decided whole, and so is one whose box lies within it, or whose points at its ends, middle and
//! quarters all do. Any other is halved, down to a floor, or until its box, the curve family's own, comes back from
//! halving all but a sixteenth as large as before. What is left are runs of pieces within the band or undecided, each
//! between two decided pieces or an end of the span, and each is read by what it lies between:
//!
//! - between the two sides, the curve crosses, and the root is found by bisection;
//! - between one side and itself, the curve's nearest approach is found by bisection on the distance's slope along it,
//!   and is a contact when it lies within the band, two crossings when it lies beyond it, and nothing when it falls
//!   short;
//! - at an end of the span that lies within the band, the curve ends on the surface: a contact;
//! - a run of pieces within the band that is the whole span lies in the surface: an overlap. Only a whole span is
//!   taken so: a run between decided pieces holds pieces that reach across an edge of the band, left open.
//!
//! The curve is moved rigidly into the form's own frame first, where the form's axis or normal is Z, so that the boxes
//! of its pieces, which follow the world's axes, follow the surface: a circle in a tilted plane then has a flat box.
//! Every side is decided through occam-math's predicates on the distance's certified interval, and one that cannot be
//! is `decide.undecided`, never a guess.

use std::sync::Arc;

use occam_geom::{Analytic, Curve};
use occam_math::{Interval, Side, Verdict};
use occam_report::Quantity;
use occam_units::Bounds;

use crate::distance::{Spread, canonical, distance, gradient, judge, padded, shrank};
use crate::hits::{Kind, Sink, Span};
use crate::methods::{Spent, Work};

/// How many times a piece is halved at most: to 2^-24 of the span.
const FLOOR: u32 = 24;

/// How many rounds a bisection takes at most, past which an interval of doubles has closed.
const ROUNDS: u32 = 128;

/// The fractions of a piece its points are tested at to call it within the band, the middle first.
const PROBES: [f64; 5] = [0.5, 0.0, 1.0, 0.25, 0.75];

/// How a piece of the span lies against the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lie {
    /// Certainly on one side of the band.
    Beside(Side),
    /// Within the band, at its box or at its probes.
    On,
    /// Not decided by the floor.
    Open,
}

/// A run of pieces within the band or undecided, from `lo` to `hi`, and what lies before it.
#[derive(Clone, Copy, Debug)]
struct Run {
    lo: f64,
    hi: f64,
    /// Whether every piece of it is within the band.
    on: bool,
    /// The side of the piece before it, or `None` at the start of the span.
    before: Option<Side>,
}

/// A curve against an analytic form, and the run being read.
struct Along<'s, 'a> {
    form: Analytic,
    /// The curve moved into the form's frame, where its boxes follow the surface, if it could be.
    moved: Option<Arc<dyn Curve>>,
    span: Span,
    sink: &'s mut Sink<'a>,
    /// The run not yet closed by a decided piece.
    open: Option<Run>,
    /// The side of the last decided piece.
    last: Option<Side>,
}

/// The hits of the sink's curve over `span` on `form`, the sink's surface's analytic form. Each run is read as soon as
/// a decided piece closes it, so a spent budget keeps what the runs before it found.
pub(crate) fn along(form: &Analytic, span: Span, work: &mut Work, sink: &mut Sink<'_>) -> Result<(), Spent> {
    let (moved, form) = canonical(sink.curve, form).map_or((None, *form), |(moved, form)| (Some(moved), form));
    let mut job = Along { form, moved, span, sink, open: None, last: None };
    job.isolate(work)?;
    if let Some(run) = job.open.take() {
        job.run(run, None);
    }
    Ok(())
}

/// The middle of `lo` and `hi`, which overflows for no finite pair.
fn middle(lo: f64, hi: f64) -> f64 {
    lo * 0.5 + hi * 0.5
}

/// `+1` for the side above the band, `-1` below it.
fn sign(side: Side) -> f64 {
    if side == Side::Below { -1.0 } else { 1.0 }
}

/// The other side.
fn opposite(side: Side) -> Side {
    if side == Side::Below { Side::Above } else { Side::Below }
}

impl Along<'_, '_> {
    fn curve(&self) -> &dyn Curve {
        self.moved.as_deref().unwrap_or(self.sink.curve)
    }

    fn band(&self) -> f64 {
        self.sink.tol.band().millimetres()
    }

    /// The signed distance of the curve's point at `t`, over the box it is known to within.
    fn at(&self, t: f64) -> Interval {
        distance(&self.form, Spread::at(self.curve().point(t)))
    }

    /// Where `value` lies against the band, counted if decided.
    fn decide(&mut self, value: Interval) -> Verdict {
        let (verdict, tally) = judge(value, self.band());
        self.sink.tally(tally);
        verdict
    }

    /// The pieces of the span in order, each decided whole or left open at the floor, and taken as they come.
    fn isolate(&mut self, work: &mut Work) -> Result<(), Spent> {
        let mut stack: Vec<(f64, f64, u32, Option<Bounds>)> = vec![(self.span.lo, self.span.hi, 0, None)];
        while let Some((lo, hi, depth, parent)) = stack.pop() {
            work.spend(1)?;
            let piece = self.curve().bounds(lo, hi);
            let held = if self.moved.is_some() { padded(&piece) } else { piece };
            let (verdict, tally) = judge(distance(&self.form, Spread::of(&held)), self.band());
            let mid = middle(lo, hi);
            let lie = match verdict {
                Verdict::Decided(side) => {
                    self.sink.tally(tally);
                    if side == Side::Within { Lie::On } else { Lie::Beside(side) }
                }
                Verdict::Undecided if depth >= FLOOR || !(lo < mid && mid < hi) => Lie::Open,
                Verdict::Undecided if self.probed_on(lo, hi) => Lie::On,
                Verdict::Undecided if parent.is_some_and(|was| !shrank(&was, &piece)) => Lie::Open,
                Verdict::Undecided => {
                    let deeper = depth.saturating_add(1);
                    stack.push((mid, hi, deeper, Some(piece)));
                    stack.push((lo, mid, deeper, Some(piece)));
                    continue;
                }
            };
            self.take(lo, hi, lie);
        }
        Ok(())
    }

    /// Whether the curve is within the band at every probe of the piece from `lo` to `hi`, the first that is not ending
    /// the test.
    fn probed_on(&mut self, lo: f64, hi: f64) -> bool {
        PROBES.iter().all(|f| {
            let t = lo + (hi - lo) * f;
            let value = self.at(t);
            self.decide(value) == Verdict::Decided(Side::Within)
        })
    }

    /// The next piece, from `lo` to `hi`: a decided one closes the open run, which is read, and any other extends it.
    fn take(&mut self, lo: f64, hi: f64, lie: Lie) {
        match (lie, self.open.as_mut()) {
            (Lie::Beside(side), _) => {
                if let Some(run) = self.open.take() {
                    self.run(run, Some(side));
                }
                self.last = Some(side);
            }
            (_, Some(run)) => {
                run.hi = hi;
                run.on &= lie == Lie::On;
            }
            (_, None) => self.open = Some(Run { lo, hi, on: lie == Lie::On, before: self.last }),
        }
    }

    /// The side at `t`, an end of the span, decided and counted, or `known` where a decided piece gives it.
    fn side(&mut self, t: f64, known: Option<Side>) -> Verdict {
        known.map_or_else(
            || {
                let value = self.at(t);
                self.decide(value)
            },
            Verdict::Decided,
        )
    }

    /// One run, between `run.before` and `after`. A run of pieces within the band that is the whole span lies in the
    /// surface: an overlap.
    fn run(&mut self, run: Run, after: Option<Side>) {
        if run.on && run.before.is_none() && after.is_none() {
            return self.sink.overlap(run.lo, run.hi);
        }
        let (start, end) = (self.side(run.lo, run.before), self.side(run.hi, after));
        match (start, end) {
            (Verdict::Decided(a), Verdict::Decided(b)) if a != Side::Within && b != Side::Within && a != b => {
                self.crossing(run.lo, run.hi, a);
            }
            (Verdict::Decided(a), Verdict::Decided(b)) if a != Side::Within && a == b => self.nearest(run, a),
            _ => {
                for (t, verdict) in [(run.lo, start), (run.hi, end)] {
                    match verdict {
                        Verdict::Decided(Side::Within) => self.sink.point(Kind::Contact, t, None),
                        Verdict::Undecided => self.undecided(t),
                        Verdict::Decided(_) => {}
                    }
                }
            }
        }
    }

    /// Reports the distance at `t` as undecided.
    fn undecided(&mut self, t: f64) {
        let (value, band) = (self.at(t), self.band());
        self.sink.undecided(t, value, Quantity::Length, band);
    }

    /// A crossing from `lo`, on `start`, to `hi`, on the other side: its root by bisection, kept where it is decided
    /// within the band.
    fn crossing(&mut self, lo: f64, hi: f64, start: Side) {
        let t = self.root(lo, hi, start);
        let value = self.at(t);
        if self.decide(value) == Verdict::Decided(Side::Within) {
            self.sink.point(Kind::Crossing, t, None);
        } else {
            self.sink.undecided(t, value, Quantity::Length, self.band());
        }
    }

    /// The root of the distance between `lo`, where it is on `start`, and `hi`, where it is on the other side, by
    /// bisection on the sign of its middle, to the doubles' resolution.
    fn root(&self, lo: f64, hi: f64, start: Side) -> f64 {
        let (mut lo, mut hi, sign) = (lo, hi, sign(start));
        for _ in 0..ROUNDS {
            let mid = middle(lo, hi);
            if !(lo < mid && mid < hi) {
                break;
            }
            if sign * self.at(mid).mid() > 0.0 { lo = mid } else { hi = mid }
        }
        middle(lo, hi)
    }

    /// A run between two pieces on `side`: the curve's nearest approach to the surface over it, decided.
    fn nearest(&mut self, run: Run, side: Side) {
        let t = self.closest(run.lo, run.hi, sign(side));
        let value = self.at(t);
        match self.decide(value) {
            Verdict::Decided(Side::Within) => self.sink.point(Kind::Contact, t, None),
            Verdict::Decided(found) if found == side => {}
            Verdict::Decided(_) => {
                self.crossing(run.lo, t, side);
                self.crossing(t, run.hi, opposite(side));
            }
            Verdict::Undecided => self.sink.undecided(t, value, Quantity::Length, self.band()),
        }
    }

    /// Where `sign` times the distance is least from `lo` to `hi`: where its slope along the curve, the surface's
    /// normal at the foot against the curve's tangent, turns from falling to rising, by bisection to the doubles'
    /// resolution. The slope is plain there, where the distance is flat, so this finds the point far closer than the
    /// distance's own least value could. An end where it does not turn.
    fn closest(&self, lo: f64, hi: f64, sign: f64) -> f64 {
        let slope = |t: f64| {
            let (point, along) = self.curve().first(t);
            sign * gradient(&self.form, point).dot(along.rate())
        };
        let falls = |t: f64| slope(t) < 0.0;
        if !falls(lo) {
            return lo;
        }
        if falls(hi) {
            return hi;
        }
        let (mut lo, mut hi) = (lo, hi);
        for _ in 0..ROUNDS {
            let mid = middle(lo, hi);
            if !(lo < mid && mid < hi) {
                break;
            }
            if falls(mid) { lo = mid } else { hi = mid }
        }
        middle(lo, hi)
    }
}
