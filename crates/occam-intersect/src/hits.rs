//! Source: original
//! Where a curve over a span meets a surface over a patch of its domain, for the boolean's edges against faces (P8) and
//! its rays (O25). [`Hits`] sits beside [`crate::Section`]: what was found, the failures met on the way, and the tally
//! of decisions. A crossing is where the curve passes through the surface, a contact where it meets the surface without
//! passing through (a touch, or an end of the span on it), and an overlap a part of the span lying in the surface.
//!
//! [`hits`] tests the curve's box over the span against the surface's over the patch first, and [`scan`] is the same
//! without that test, so that the two runs can be compared: the box test must change nothing (P8). A surface that
//! presents an analytic form is scanned by its signed distance along the curve, and any other by subdividing the span
//! and the patch together. Points are kept where their parameters lie in the patch, edges included, and overlaps where
//! their box meets the patch's, which is what makes the box test conservative.

use std::sync::Arc;

use occam_geom::{Curve, Domain, ParamRange, Scratch, Surface, UvBox, Wrap};
use occam_math::{Exec, Grain, Interval, Tally};
use occam_report::{Code, Fault, NamedValue, Quantity, Reason, Report};
use occam_units::{Point, Tolerance, UvPoint, World};

use crate::methods::{Budget, Work};
use crate::{along, subdivide};

/// The part of a curve's range from `lo` to `hi`, `lo` below `hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    /// Where it starts.
    pub lo: f64,
    /// Where it ends.
    pub hi: f64,
}

/// A point where the curve meets the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The curve's parameter there.
    pub t: f64,
    /// The curve's point there.
    pub at: Point<World>,
    /// The surface's parameters at the foot of the point, inside the patch.
    pub uv: UvPoint,
}

/// A part of the span over which the curve lies in the surface, within the band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlap {
    /// Where it starts.
    pub lo: f64,
    /// Where it ends.
    pub hi: f64,
}

/// What a curve over a span gives against a surface over a patch. A failure never takes the place of what was found:
/// both are kept.
#[derive(Clone, Debug, Default)]
pub struct Hits {
    /// Where the curve passes through the surface, in the order of the curve's parameter.
    pub crossings: Vec<Hit>,
    /// Where it meets the surface without passing through: a touch, or an end of the span on the surface. In order.
    pub contacts: Vec<Hit>,
    /// Where it lies in the surface, in order.
    pub overlaps: Vec<Overlap>,
    /// A spent budget, an undecided predicate, a span or patch outside the domain: what was found stays above.
    pub failures: Vec<Report>,
    /// The decisions made.
    pub tally: Tally,
}

/// A curve over a span against a surface over a patch, shared, so that many pairs can run as tasks.
#[derive(Clone)]
pub struct HitPair {
    /// The curve.
    pub curve: Arc<dyn Curve>,
    /// The part of its range asked about.
    pub span: Span,
    /// The surface.
    pub surface: Arc<dyn Surface>,
    /// The part of its domain asked about, such as the box of a face's trims.
    pub patch: UvBox,
}

/// Whether the curve's box over `span` comes within the band of the surface's box over `patch`. Where it does not, no
/// hit can be found, and [`hits`] skips the scan. It may say yes for boxes a little further apart, never no for boxes
/// within the band.
#[must_use]
pub fn boxes_meet(curve: &dyn Curve, span: Span, surface: &dyn Surface, patch: UvBox, tol: Tolerance) -> bool {
    curve.bounds(span.lo, span.hi).overlaps(&surface.bounds(patch), tol)
}

/// The hits of `curve` over `span` on `surface` over `patch`, once their boxes meet ([`boxes_meet`]): otherwise none,
/// with no work done. A span or patch outside the domain is refused first, as [`scan`] refuses it, so the two are the
/// same wherever the boxes are conservative.
#[must_use]
pub fn hits(
    curve: &dyn Curve,
    span: Span,
    surface: &dyn Surface,
    patch: UvBox,
    tol: Tolerance,
    budget: Budget,
) -> Hits {
    let refused = valid(curve.range(), span, surface.domain(), patch).is_err();
    if refused || boxes_meet(curve, span, surface, patch, tol) {
        scan(curve, span, surface, patch, tol, budget)
    } else {
        Hits::default()
    }
}

/// The hits of `curve` over `span` on `surface` over `patch`, with no box test first. A span or patch outside the
/// domain is reported and nothing scanned; a scan that runs out of `budget` gives `intersect.hits-incomplete` beside
/// what it found.
#[must_use]
pub fn scan(
    curve: &dyn Curve,
    span: Span,
    surface: &dyn Surface,
    patch: UvBox,
    tol: Tolerance,
    budget: Budget,
) -> Hits {
    let mut sink = Sink {
        curve,
        surface,
        patch,
        domain: surface.domain(),
        tol,
        scratch: Scratch::default(),
        hits: Hits::default(),
    };
    if let Err(report) = valid(curve.range(), span, sink.domain, patch) {
        sink.hits.failures.push(report);
        return sink.hits;
    }
    let mut work = Work::of(budget);
    let ran = match surface.analytic(tol) {
        Some(form) => along::along(&form, span, &mut work, &mut sink),
        None => subdivide::subdivide(span, &mut work, &mut sink),
    };
    if ran.is_err() {
        let found = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let points = found(sink.hits.crossings.len().saturating_add(sink.hits.contacts.len()));
        let report = Report::new(Code::HitsIncomplete, Fault::NumericallySensitive)
            .with_reason(Reason::BudgetSpent)
            .with_value(NamedValue::count("hits", points))
            .with_value(NamedValue::count("overlaps", found(sink.hits.overlaps.len())))
            .with_value(NamedValue::count("budget", budget.steps));
        sink.hits.failures.push(report);
    }
    sink.hits
}

/// The hits of every pair, one pair a task through `exec`, one result each in pair order (P8, L6). The same pairs give
/// the same bits on any number of threads.
#[must_use]
pub fn hits_of_pairs(exec: &Exec, pairs: Arc<[HitPair]>, tol: Tolerance, budget: Budget) -> Vec<Hits> {
    let count = pairs.len();
    let found = exec.map(count, Grain::Each, move |index| {
        pairs.get(index).map(|p| hits(p.curve.as_ref(), p.span, p.surface.as_ref(), p.patch, tol, budget))
    });
    found.into_iter().flatten().collect()
}

/// Refuses a span that is empty or leaves the curve's range, and a patch that leaves the surface's domain.
fn valid(range: ParamRange, span: Span, domain: Domain, patch: UvBox) -> Result<(), Report> {
    range.check(span.lo)?;
    range.check(span.hi)?;
    range.trimmed(span.lo, span.hi)?;
    domain.check(patch.lo)?;
    domain.check(patch.hi)?;
    let ordered = patch.lo.u() <= patch.hi.u() && patch.lo.v() <= patch.hi.v();
    if ordered { Ok(()) } else { Err(Report::new(Code::UndefinedParameter, Fault::InvalidInput)) }
}

/// `x` moved into `lo..=hi` by whole periods where `range` comes round, if it can be.
fn into(x: f64, (lo, hi): (f64, f64), range: ParamRange) -> Option<f64> {
    let x = if range.wrap == Wrap::Periodic { lo + (x - lo).rem_euclid(range.hi - range.lo) } else { x };
    (lo <= x && x <= hi).then_some(x)
}

/// What a point found is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The curve passes through the surface there.
    Crossing,
    /// It meets the surface without passing through.
    Contact,
}

/// What a scan adds its findings to: it finds each point's parameters on the surface, keeps those in the patch, and
/// reports what it cannot decide.
pub(crate) struct Sink<'a> {
    pub(crate) curve: &'a dyn Curve,
    pub(crate) surface: &'a dyn Surface,
    pub(crate) patch: UvBox,
    domain: Domain,
    pub(crate) tol: Tolerance,
    pub(crate) scratch: Scratch,
    hits: Hits,
}

impl Sink<'_> {
    /// Keeps a failure met on the way.
    pub(crate) fn fail(&mut self, report: Report) {
        self.hits.failures.push(report);
    }

    /// Counts decisions made.
    pub(crate) fn tally(&mut self, tally: Tally) {
        self.hits.tally = self.hits.tally.merged(tally);
    }

    /// `uv` moved into the patch by whole turns, if it lies in it.
    pub(crate) fn in_patch(&self, uv: UvPoint) -> Option<UvPoint> {
        let (lo, hi) = (self.patch.lo, self.patch.hi);
        let u = into(uv.u(), (lo.u(), hi.u()), self.domain.u)?;
        Some(UvPoint::new(u, into(uv.v(), (lo.v(), hi.v()), self.domain.v)?))
    }

    /// The point at `t`, of `kind`, with its parameters on the surface, `uv` or the foot's: kept if they lie in the
    /// patch. A foot that cannot be found is reported.
    pub(crate) fn point(&mut self, kind: Kind, t: f64, uv: Option<UvPoint>) {
        let at = self.curve.point(t);
        let foot = match uv {
            Some(uv) => uv,
            None => match self.surface.invert(at, None, self.tol, &mut self.scratch) {
                Ok(uv) => uv,
                Err(report) => return self.fail(report),
            },
        };
        let Some(uv) = self.in_patch(foot) else { return };
        let list = if kind == Kind::Crossing { &mut self.hits.crossings } else { &mut self.hits.contacts };
        list.push(Hit { t, at, uv });
    }

    /// The curve over `lo` to `hi` lies in the surface: kept if its box meets the patch's.
    pub(crate) fn overlap(&mut self, lo: f64, hi: f64) {
        if boxes_meet(self.curve, Span { lo, hi }, self.surface, self.patch, self.tol) {
            self.hits.overlaps.push(Overlap { lo, hi });
        }
    }

    /// `decide.undecided` for `value`, a `quantity` decided at the curve's parameter `t` against `[-eps, eps]`.
    pub(crate) fn undecided(&mut self, t: f64, value: Interval, quantity: Quantity, eps: f64) {
        let at = self.curve.point(t).to_vec3();
        let report = Report::new(Code::Undecided, Fault::NumericallySensitive)
            .with_value(NamedValue::new("quantity", quantity, value.mid()))
            .with_value(NamedValue::new("radius", quantity, value.radius()))
            .with_value(NamedValue::new("band", quantity, eps))
            .with_value(NamedValue::point("at", [at.x, at.y, at.z]))
            .with_tolerance_in_force(self.tol.band().millimetres());
        self.hits.failures.push(report);
        self.hits.tally.undecided = self.hits.tally.undecided.saturating_add(1);
    }
}
