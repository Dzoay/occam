//! Source: piegl-tiller-1997
//! Traced points become curves (P5, L5). A trace is cut where a surface's parameters jump, at each seam or pole it
//! crossed, and a closed trace's last piece carries on round to its first station. Each piece becomes a B-spline
//! through some of its stations at their chord lengths, cubic where it has four, and its image in each surface's
//! parameters a B-spline through the same stations' parameters there at the same parameters, so that the three run
//! together (Piegl and Tiller, section 9.2.1, through occam-geom's `interpolate`). A parameter that comes round, which a
//! trace crosses no edge of, is carried round by whole periods to within half a period of the station before.
//!
//! Which stations is found by refining. It starts from every eighth and the last. At every station and halfway to the
//! next, two lengths for each surface are decided against the band through occam-math's predicates, and tallied: the
//! curve's distance from the surface, to first order the part along the surface's normal of the gap between the curve
//! and the surface's point at the image, twice over; and that whole gap, which is how far the image maps from the
//! curve. Refining aims at half the band: where either length, twice over, is not certainly within it, the span
//! between chosen stations gains the station halfway along it, or where it has none, a new one that the trace's
//! corrector settles onto both surfaces halfway between its ends, and the curves are made again: until every length
//! meets the aim, no span that misses can gain a station, or 16 rounds are done. The answer is the band: where a length
//! misses the aim, it is decided again as it is. Aiming at the band itself left a loop whose spans all fit at its edge
//! refined one span a round, each tipping its neighbour over, for longer than the rounds allow. New stations are for a surface parametrized with less smoothness than its shape, as
//! a rational circle is at its knots, where an image is less smooth than its curve.
//!
//! The curve's fit is twice its largest distance from a surface: doubling leaves as much again for what lies between
//! samples, so a fit that reached the band is within it. A fit or an image that did not is
//! `intersect.fit-outside-band`, with what it reached, and its curve is kept; a length the band cannot place is
//! `decide.undecided`. Neither has a remedy an agent could follow.
//!
//! An image's fit, in its surface's parameters, is twice the largest distance there, at the same samples, between the
//! image and the parameters of the foot of the curve's point on the surface: to first order, the Gauss-Newton step to
//! it from the image's, which needs no search for the foot. A parameter at a pole measures nothing, so a piece's end at
//! a pole the trace crossed is left out of that surface's, as is any place the derivatives span no plane. No fit is
//! below the rounding of numbers the size of its stations'.

use std::sync::Arc;

use occam_geom::{
    Curve, Domain, FittedCurve, ParamCurve, ParamRange, SplineCurve, Surface, UvCurve, Wrap, interpolate,
};
use occam_math::{Exec, Grain, Interval, Side, Tally};
use occam_report::{Code, Fault, NamedValue, Report};
use occam_units::{Length, Point, Tolerance, UvPoint, Vec3, World};

use crate::correct::Pair;
use crate::decide::{Decide, ahead, distance, undecided};
use crate::methods::SurfacePair;
use crate::on::On;
use crate::section::SectionCurve;
use crate::trace::{Crossing, End, Jump, Start, Station, Trace, Uncertain, Which};

/// How far apart, in stations, the first chosen stations are.
const STRIDE: usize = 8;

/// The most rounds of refining. Three halve a span of [`STRIDE`] stations to each of them; each round after that
/// settles a new station in each span that still misses, halving it.
const ROUNDS: u32 = 16;

/// The rounding of a number, relative to its size and one, below which no fit is claimed.
const ROUNDING: f64 = 64.0 * f64::EPSILON;

/// What fitting a trace gives: a curve for each of its pieces, with its images on both surfaces, beside the failures
/// met and the tally of the decisions made.
#[derive(Clone, Default)]
pub struct Fitted {
    /// The curves, each with its images, in trace order.
    pub curves: Vec<SectionCurve>,
    /// A fit that did not reach the band, with its curve kept above, and a gap the band could not place.
    pub failures: Vec<Report>,
    /// The decisions made against the band.
    pub tally: Tally,
}

/// The curves of `trace`, which traced where `a` and `b` meet, fitted to `tol`'s band.
#[must_use]
pub fn fit(a: &dyn Surface, b: &dyn Surface, trace: &Trace, tol: Tolerance) -> Fitted {
    let (pair, mut out) = (Pair::new(a, b, tol), Fitted::default());
    for piece in pieces(trace) {
        fit_piece(&pair, [a, b], &piece, &mut out);
    }
    out
}

/// The fit of each of `traces` of `pair`, one trace a task through `exec`, in trace order: the same bits on any number
/// of threads.
#[must_use]
pub fn fits(exec: &Exec, pair: &SurfacePair, traces: Arc<[Trace]>, tol: Tolerance) -> Vec<Fitted> {
    On::Shared(exec, pair).map(traces, Grain::Each, move |a, b, trace| fit(a, b, trace, tol))
}

/// A run of stations along which no surface's parameters jump, none repeating the one before it, and the surface at
/// a pole at either end, if one is.
struct Piece {
    stations: Vec<Station>,
    poles: [Option<Which>; 2],
}

/// The surface at a pole where `crossing` is.
fn pole(crossing: Option<&Crossing>) -> Option<Which> {
    crossing.filter(|crossing| crossing.jump == Jump::Pole).map(|crossing| crossing.on)
}

/// The piece of `stations` between the crossings `before` and `after`.
fn piece(stations: &[Station], before: Option<&Crossing>, after: Option<&Crossing>) -> Piece {
    let mut kept: Vec<Station> = Vec::with_capacity(stations.len());
    for station in stations {
        if kept.last().is_none_or(|last| last.at != station.at) {
            kept.push(*station);
        }
    }
    Piece { stations: kept, poles: [pole(before), pole(after)] }
}

/// `trace` cut at each crossing, a closed trace's last piece carried on round to its first station.
fn pieces(trace: &Trace) -> Vec<Piece> {
    let stations = trace.stations.as_slice();
    let (mut out, mut from, mut before) = (Vec::new(), 0, None);
    for crossing in &trace.crossings {
        out.push(piece(stations.get(from..=crossing.at).unwrap_or_default(), before, Some(crossing)));
        (from, before) = (crossing.at.saturating_add(1), Some(crossing));
    }
    let rest = stations.get(from..).unwrap_or_default();
    match (trace.ends.contains(&End::Closed), trace.crossings.first()) {
        (true, Some(first)) => {
            let head = stations.get(..=first.at).unwrap_or_default();
            let round: Vec<Station> = rest.iter().chain(head).copied().collect();
            if let Some(slot) = out.first_mut() {
                *slot = piece(&round, before, Some(first));
            }
        }
        (true, None) => {
            let round: Vec<Station> = rest.iter().chain(stations.first()).copied().collect();
            out.push(piece(&round, None, None));
        }
        (false, _) => out.push(piece(rest, before, None)),
    }
    out
}

/// The point of the plane `z = 0` at `uv`, as an image is drawn.
fn lifted(uv: UvPoint) -> Point<World> {
    Point::from_vec3(Vec3::new(uv.u(), uv.v(), 0.0))
}

/// The parameters an image's point stands for.
fn flat(p: Point<World>) -> UvPoint {
    let v = p.to_vec3();
    UvPoint::new(v.x, v.y)
}

/// `x`, a value of `range`'s parameter, moved by whole periods to within half a period of `before` where the parameter
/// comes round.
fn round_to(x: f64, before: f64, range: ParamRange) -> f64 {
    let period = range.hi - range.lo;
    if range.wrap != Wrap::Periodic || !(period.is_finite() && period > 0.0) {
        return x;
    }
    x - period * ((x - before) / period).round()
}

/// `uv` with each parameter that comes round in `domain` carried to within half a period of `before`'s.
fn carry(uv: UvPoint, before: UvPoint, domain: Domain) -> UvPoint {
    UvPoint::new(round_to(uv.u(), before.u(), domain.u), round_to(uv.v(), before.v(), domain.v))
}

/// `stations` with their parameters on each surface carried round to the station before's, so that an image does not
/// jump where a loop comes round to its first station a whole turn on.
fn carried(stations: &[Station], [a, b]: [Domain; 2]) -> Vec<Station> {
    let mut out: Vec<Station> = Vec::with_capacity(stations.len());
    for station in stations {
        let next = match out.last() {
            Some(before) => Station {
                on_a: carry(station.on_a, before.on_a, a),
                on_b: carry(station.on_b, before.on_b, b),
                ..*station
            },
            None => *station,
        };
        out.push(next);
    }
    out
}

/// A piece's stations as the curves are made through them: chord lengths, points, and parameters on each surface.
struct Data {
    params: Vec<f64>,
    points: Vec<Point<World>>,
    on: [Vec<Point<World>>; 2],
}

impl Data {
    fn of(stations: &[Station]) -> Self {
        let (mut length, mut before) = (0.0, None);
        let params = stations
            .iter()
            .map(|station| {
                let here = station.at.to_vec3();
                if let Some(before) = before.replace(here) {
                    length += (here - before).norm();
                }
                length
            })
            .collect();
        let on = |pick: fn(&Station) -> UvPoint| stations.iter().map(|s| lifted(pick(s))).collect();
        let points = stations.iter().map(|s| s.at).collect();
        Self { params, points, on: [on(|s| s.on_a), on(|s| s.on_b)] }
    }
}

/// The curve and its two images through the chosen stations.
struct Through {
    curve: SplineCurve,
    images: [SplineCurve; 2],
}

/// The curves through the stations `chosen`, cubic where there are four.
fn through(data: &Data, chosen: &[usize]) -> Result<Through, Report> {
    let params: Vec<f64> = chosen.iter().filter_map(|&k| data.params.get(k).copied()).collect();
    let degree = u16::try_from(chosen.len().saturating_sub(1).min(3)).unwrap_or(3);
    let make = |all: &[Point<World>]| {
        let points: Vec<Point<World>> = chosen.iter().filter_map(|&k| all.get(k).copied()).collect();
        SplineCurve::new(&interpolate(degree, &params, &points)?)
    };
    let [on_a, on_b] = &data.on;
    Ok(Through { curve: make(&data.points)?, images: [make(on_a)?, make(on_b)?] })
}

/// Where the curves are measured: every station, each with the index of the span to the next, and halfway to the
/// next; the last station counts in the span before it.
fn samples(params: &[f64]) -> Vec<(f64, usize)> {
    let mut out = Vec::with_capacity(params.len().saturating_mul(2));
    for (k, pair) in params.windows(2).enumerate() {
        if let &[t, next] = pair {
            out.extend([(t, k), (f64::midpoint(t, next), k)]);
        }
    }
    out.extend(params.last().map(|&last| (last, params.len().saturating_sub(2))));
    out
}

/// The largest of a kind of gap found, NaN once one is, and the curve's point there.
#[derive(Clone, Copy)]
struct Worst {
    gap: f64,
    at: Vec3,
}

impl Worst {
    /// None yet.
    const NONE: Self = Self { gap: 0.0, at: Vec3::new(0.0, 0.0, 0.0) };

    /// Notes a gap enclosed by `gap`, its size the far end of the enclosure from zero, at `at`.
    fn note(&mut self, gap: Interval, at: Vec3) {
        let size = gap.mid().abs() + gap.radius();
        if !self.gap.is_nan() && (size.is_nan() || size > self.gap) {
            *self = Self { gap: size, at };
        }
    }
}

/// What a round of samples found.
struct Found {
    /// The spans between chosen stations, by index and in order, where a gap was not certainly within the band.
    missed: Vec<usize>,
    /// The curve's largest distance from a surface.
    off: Worst,
    /// The largest gap between the curve and a surface's point at the image.
    image: Worst,
    /// Whether a gap was certainly outside the band.
    outside: bool,
    /// The first gap the band could not place.
    undecided: Option<Uncertain>,
}

impl Found {
    /// Nothing found yet.
    const fn new() -> Self {
        Self { missed: Vec::new(), off: Worst::NONE, image: Worst::NONE, outside: false, undecided: None }
    }

    /// Decides `length` against the band at `at`: whether it is certainly within it.
    fn within(&mut self, decide: &mut Decide, length: Interval, at: Vec3) -> bool {
        match decide.side(length, at) {
            Ok(Side::Within) => true,
            Ok(Side::Below | Side::Above) => {
                self.outside = true;
                false
            }
            Err(uncertain) => {
                self.undecided.get_or_insert(uncertain);
                false
            }
        }
    }

    /// Whether `length` at `at`, twice over, is certainly within the band, as refining aims: within half of it, so that
    /// a span refined beside one does not tip it over the band. Where it is not, `length` itself is decided as the
    /// answer, and the span is refined. An aim the band cannot place is refined too, and only the answer reported.
    fn aim(&mut self, decide: &mut Decide, length: Interval, at: Vec3) -> bool {
        if decide.side(Interval::exact(2.0) * length, at) == Ok(Side::Within) {
            return true;
        }
        self.within(decide, length, at);
        false
    }

    /// Notes that span `span` missed.
    fn miss(&mut self, span: usize) {
        if self.missed.last() != Some(&span) {
            self.missed.push(span);
        }
    }
}

/// How far `at` is from `surface`, from its point at `uv`: to first order, along the unit normal there, or all the way
/// where it has none; and the whole gap between the two.
fn gaps(surface: &dyn Surface, uv: UvPoint, at: Vec3) -> (Interval, Interval) {
    let jet = surface.first(uv);
    let (on, normal) = (jet.point.to_vec3(), jet.du.rate().cross(jet.dv.rate()));
    let (whole, length) = (distance(on, at), normal.norm());
    let off = if length > 0.0 && length.is_finite() { ahead(on, at, normal * (1.0 / length)) } else { whole };
    (off, whole)
}

/// The curve's distance from each surface at `samples`, twice over, and the gap between it and each surface's point at
/// its image, each decided against the band as refining aims and, where it misses, as the answer.
fn measure(
    surfaces: [&dyn Surface; 2],
    through: &Through,
    (samples, chosen): (&[(f64, usize)], &[usize]),
    decide: &mut Decide,
) -> Found {
    let mut found = Found::new();
    let last = chosen.len().saturating_sub(2);
    for &(t, k) in samples {
        let at = through.curve.point(t).to_vec3();
        let mut missed = false;
        for (surface, image) in surfaces.iter().zip(&through.images) {
            let (off, whole) = gaps(*surface, flat(image.point(t)), at);
            found.off.note(off, at);
            found.image.note(whole, at);
            let near = found.aim(decide, Interval::exact(2.0) * off, at);
            let maps = found.aim(decide, whole, at);
            missed |= !(near && maps);
        }
        if missed {
            found.miss(chosen.partition_point(|&c| c <= k).saturating_sub(1).min(last));
        }
    }
    found
}

/// The stations chosen first: every [`STRIDE`]th, and the last.
fn first_chosen(count: usize) -> Vec<usize> {
    let last = count.saturating_sub(1);
    (0..last).step_by(STRIDE).chain([last]).collect()
}

/// A station between `before` and `after` where both surfaces meet, settled by the trace's corrector in the plane
/// halfway between them square to the chord, from parameters halfway between theirs. `None` where it does not settle,
/// or settles outside a surface's domain.
fn between(pair: &Pair<'_>, before: &Station, after: &Station) -> Option<Station> {
    let (from, to) = (before.at.to_vec3(), after.at.to_vec3());
    let chord = to - from;
    let half = |x: UvPoint, y: UvPoint| UvPoint::new(f64::midpoint(x.u(), y.u()), f64::midpoint(x.v(), y.v()));
    let seed = pair.seed(Start { on_a: half(before.on_a, after.on_a), on_b: half(before.on_b, after.on_b) });
    let settled = pair.station(&pair.correct(&seed, (from + to) * 0.5, chord * (1.0 / chord.norm()))?);
    let (a, b) = if pair.swapped { (pair.q, pair.p) } else { (pair.p, pair.q) };
    (a.domain().contains(settled.on_a) && b.domain().contains(settled.on_b)).then_some(settled)
}

/// Each span of `chosen` in `missed` gains the station halfway along it, or where it has none, a new one settled
/// between its ends: `stations` and `chosen` as they become, and whether any span gained one.
fn refine(pair: &Pair<'_>, stations: &mut Vec<Station>, chosen: &mut Vec<usize>, missed: &[usize]) -> bool {
    let room = missed.len();
    let (mut kept, mut picked) = (Vec::with_capacity(stations.len().saturating_add(room)), Vec::new());
    let mut changed = false;
    for (j, span) in chosen.windows(2).enumerate() {
        let &[from, to] = span else { continue };
        let base = kept.len();
        picked.push(base);
        kept.extend(stations.get(from..to).unwrap_or_default());
        if missed.binary_search(&j).is_err() {
            continue;
        }
        if to.saturating_sub(from) >= 2 {
            picked.push(base.saturating_add(from.midpoint(to).saturating_sub(from)));
            changed = true;
        } else if let Some(new) = stations.get(from).zip(stations.get(to)).and_then(|(b, a)| between(pair, b, a)) {
            picked.push(kept.len());
            kept.push(new);
            changed = true;
        }
    }
    picked.push(kept.len());
    kept.extend(stations.last());
    (*stations, *chosen) = (kept, picked);
    changed
}

/// `fit`, or where it is smaller, the rounding of numbers as large as the largest coordinate of `points`.
fn floored(fit: f64, points: &[Point<World>]) -> f64 {
    let size = points.iter().flat_map(|p| {
        let v = p.to_vec3();
        [v.x.abs(), v.y.abs(), v.z.abs()]
    });
    let floor = ROUNDING * (1.0 + size.fold(0.0, |most, x| if x > most { x } else { most }));
    if fit.is_nan() || fit > floor { fit } else { floor }
}

/// How far in the parameters the foot of `p` on `surface` is from `uv`, to first order: the length of the Gauss-Newton
/// step `(J^T J)^-1 J^T (p - S(uv))`, where `J` holds the derivatives at `uv`. `None` where they span no plane, as at a
/// pole.
fn step(surface: &dyn Surface, uv: UvPoint, p: Vec3) -> Option<f64> {
    let jet = surface.first(uv);
    let (su, sv, r) = (jet.du.rate(), jet.dv.rate(), p - jet.point.to_vec3());
    let (uu, mixed, vv) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let det = uu * vv - mixed * mixed;
    if !(det > 0.0 && det.is_finite()) {
        return None;
    }
    let (ru, rv) = (su.dot(r), sv.dot(r));
    let (du, dv) = ((vv * ru - mixed * rv) / det, (uu * rv - mixed * ru) / det);
    Some((du * du + dv * dv).sqrt())
}

/// The fit of `image`, `curve`'s image on `surface`, at the parameters `at`: twice the largest distance there between
/// the image and the foot of the curve's point, NaN once one is.
fn image_fit(surface: &dyn Surface, (image, curve): (&SplineCurve, &SplineCurve), at: &[(f64, usize)]) -> f64 {
    let mut worst = 0.0_f64;
    for &(t, _) in at {
        let Some(d) = step(surface, flat(image.point(t)), curve.point(t).to_vec3()) else { continue };
        if !worst.is_nan() && (d.is_nan() || d > worst) {
            worst = d;
        }
    }
    2.0 * worst
}

/// The fit of each of `made`'s images over `samples`, but for a piece's end at a pole of the image's surface.
fn image_fits(
    surfaces: [&dyn Surface; 2],
    (piece, data): (&Piece, &Data),
    made: &Through,
    samples: &[(f64, usize)],
) -> [f64; 2] {
    let ([a, b], [on_a, on_b], [image_a, image_b]) = (surfaces, &data.on, &made.images);
    let [first, last] = piece.poles;
    [(Which::A, a, image_a, on_a), (Which::B, b, image_b, on_b)].map(|(which, surface, image, on)| {
        let from = usize::from(first == Some(which));
        let to = samples.len().saturating_sub(usize::from(last == Some(which)));
        let at = samples.get(from..to).unwrap_or_default();
        floored(image_fit(surface, (image, &made.curve), at), on)
    })
}

/// The report of a fit that did not reach the band: what it reached, over how many stations, and where it is furthest.
fn outside(reached: f64, band: f64, points: usize, at: Vec3) -> Report {
    Report::new(Code::FitOutsideBand, Fault::NumericallySensitive)
        .with_value(NamedValue::count("points", u32::try_from(points).unwrap_or(u32::MAX)))
        .with_value(NamedValue::length("reached", reached))
        .with_value(NamedValue::length("band", band))
        .with_value(NamedValue::point("at", [at.x, at.y, at.z]))
        .with_tolerance_in_force(band)
}

/// The section curve of `through`, with the fits given.
fn section_curve(through: Through, fit: f64, [fit_a, fit_b]: [f64; 2]) -> Result<SectionCurve, Report> {
    let Through { curve, images: [a, b] } = through;
    let image = |spline: SplineCurve, fit: f64| -> Result<Arc<dyn ParamCurve>, Report> {
        Ok(Arc::new(UvCurve::new(FittedCurve::new(spline, Length::from_millimetres(fit))?)?))
    };
    let curve: Arc<dyn Curve> = Arc::new(FittedCurve::new(curve, Length::from_millimetres(fit))?);
    Ok(SectionCurve { curve, on_a: image(a, fit_a)?, on_b: image(b, fit_b)? })
}

/// The last round of refining: the stations as they became, the samples, the curves and what the samples found.
struct Last {
    data: Data,
    samples: Vec<(f64, usize)>,
    made: Through,
    found: Found,
}

/// `stations` refined until every gap is within half the band, no span that misses can gain a station, or [`ROUNDS`]
/// rounds are done: the last round.
fn refined(
    surfaces: [&dyn Surface; 2],
    pair: &Pair<'_>,
    mut stations: Vec<Station>,
    decide: &mut Decide,
) -> Result<Last, Report> {
    let (mut chosen, mut rounds) = (first_chosen(stations.len()), 1);
    loop {
        let data = Data::of(&stations);
        let samples = samples(&data.params);
        let made = through(&data, &chosen)?;
        let found = measure(surfaces, &made, (&samples, &chosen), decide);
        if found.missed.is_empty() || rounds >= ROUNDS || !refine(pair, &mut stations, &mut chosen, &found.missed) {
            return Ok(Last { data, samples, made, found });
        }
        rounds = rounds.saturating_add(1);
    }
}

/// The curve of `piece` and its images, into `out`, beside what failed.
fn fit_piece(pair: &Pair<'_>, surfaces: [&dyn Surface; 2], piece: &Piece, out: &mut Fitted) {
    if piece.stations.len() < 2 {
        return;
    }
    let band = pair.band();
    let stations = carried(&piece.stations, surfaces.map(Surface::domain));
    let mut decide = Decide::new(band);
    let refined = refined(surfaces, pair, stations, &mut decide);
    out.tally = out.tally.merged(decide.tally);
    let Last { data, samples, made, found } = match refined {
        Ok(last) => last,
        Err(report) => return out.failures.push(report),
    };
    let fit = floored(2.0 * found.off.gap, &data.points);
    if found.outside {
        let image = found.image;
        let (reached, at) = if image.gap > fit { (image.gap, image.at) } else { (fit, found.off.at) };
        out.failures.push(outside(reached, band, data.points.len(), at));
    }
    out.failures.extend(found.undecided.map(|uncertain| undecided(&uncertain, band)));
    let fits = image_fits(surfaces, (piece, &data), &made, &samples);
    match section_curve(made, fit, fits) {
        Ok(curve) => out.curves.push(curve),
        Err(report) => out.failures.push(report),
    }
}

#[cfg(test)]
mod unit;
