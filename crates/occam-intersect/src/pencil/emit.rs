//! Source: original
//! What the pencil method puts in the section: each class's curves, made, with their images on both surfaces, and its
//! singular points as contacts. Lines and conics are made and imaged by the closed forms' own code. A quartic's loop and
//! a rational quartic's or cubic's B-spline are read through each surface's chart by occam-geom's image family,
//! counting the turns they make about the chart's axis, and each is first cut where it passes a point at which a
//! chart's parameters are undefined: a sphere's pole or a cone's apex.
//!
//! A section that reaches infinity is given within the extent asked, in bounded pieces that reach past it: a quartic's
//! loops where their ruling parameter is within its reach, and a rational curve's stretches inside its ball. A line
//! stays whole, as a line is everywhere else.

use std::f64::consts::TAU;
use std::sync::Arc;

use occam_geom::{Chart, Curve, Image, ParametricCurve, TrimmedCurve, Wrap};
use occam_math::Side;
use occam_units::{Point, Vec3};

use super::fan::{Ball, Fan};
use super::quadric::{Kind, Natural, Ruling};
use super::{Class, Pair, nodal, smooth, split};
use crate::closed::{Found, Plan, emit, exact3, minus, norm3};
use crate::decide::{Decisions, Undecided};
use crate::section::{Contact, Section, SectionCurve};

/// The points of the world at which a chart's parameters are undefined: the charts' poles and the cones' apexes.
fn undefined(pair: &Pair, charts: [&Chart; 2]) -> Vec<Vec3> {
    let poles = charts.iter().flat_map(|c| c.poles()).map(Point::to_vec3);
    let apexes = pair.surfaces.iter().filter_map(|s| match s {
        Natural::Cone { apex, .. } => Some(*apex),
        _ => None,
    });
    poles.chain(apexes).collect()
}

/// The samples a curve's range is searched in for where it passes a point.
const SAMPLES: u32 = 64;

/// The parameters from `lo` to `hi` where `curve` comes nearest `p`, each with the sample it follows: where the slope
/// of the squared distance, `(c(t) − p) · c'(t)`, rises through zero between two samples, each found by halving, which
/// keeps a middle where the slope is zero as an end, whichever half it keeps.
fn nearest(curve: &dyn Curve, p: Vec3, (lo, hi): (f64, f64)) -> Vec<(f64, u32)> {
    let slope = |t: f64| {
        let (at, rate) = curve.first(t);
        (at.to_vec3() - p).dot(rate.rate())
    };
    let step = (hi - lo) / f64::from(SAMPLES);
    let mut out = Vec::new();
    for i in 0..SAMPLES {
        let (mut a, mut b) = (lo + step * f64::from(i), lo + step * f64::from(i.saturating_add(1)));
        if !(slope(a) < 0.0 && slope(b) >= 0.0) {
            continue;
        }
        for _ in 0..80 {
            let mid = 0.5 * a + 0.5 * b;
            if slope(mid).is_sign_negative() { a = mid } else { b = mid }
        }
        out.push((0.5 * a + 0.5 * b, i));
    }
    out
}

/// The parameters at which `curve` passes through one of `points`, its point there within the band of it, however
/// often: but not where an open curve already ends, in the first or last sample and with that end within the band,
/// since a curve at rest there, as at a cusp, finds its nearest point only to the square root of the rounding in its
/// parameter.
fn cuts(curve: &dyn Curve, points: &[Vec3], decide: &mut Decisions) -> Result<Vec<f64>, Undecided> {
    let range = curve.range();
    let open = range.wrap != Wrap::Periodic;
    let ends = [(0, range.lo), (SAMPLES.saturating_sub(1), range.hi)].map(|(i, t)| (i, curve.point(t).to_vec3()));
    let apart = |a: Vec3, b: Vec3| norm3(minus(exact3(a), exact3(b)));
    let mut out = Vec::new();
    for p in points {
        for (t, sample) in nearest(curve, *p, (range.lo, range.hi)) {
            let at = curve.point(t).to_vec3();
            if decide.length(apart(at, *p))? != Side::Within {
                continue;
            }
            let mut ended = false;
            for (i, end) in ends {
                ended |= open && i == sample && decide.length(apart(end, at))? == Side::Within;
            }
            if !ended {
                out.push(t);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    Ok(out)
}

/// The parts of a range cut at `cuts`, each from one cut to the next, round the period where it comes round.
fn parts(range: occam_geom::ParamRange, cuts: &[f64]) -> Vec<(f64, f64)> {
    if range.wrap == Wrap::Periodic {
        let (Some(first), Some(last)) = (cuts.first().copied(), cuts.last().copied()) else { return Vec::new() };
        let mut out: Vec<(f64, f64)> = cuts.windows(2).filter_map(|w| Some((*w.first()?, *w.last()?))).collect();
        out.push((last, first + (range.hi - range.lo)));
        return out;
    }
    let ends: Vec<f64> = std::iter::once(range.lo).chain(cuts.iter().copied()).chain([range.hi]).collect();
    ends.windows(2).filter_map(|w| Some((*w.first()?, *w.last()?))).collect()
}

/// `curve` with its images through both charts, or `None` where one cannot be read.
fn imaged<C: ParametricCurve + Clone>(curve: C, charts: [&Chart; 2]) -> Option<SectionCurve> {
    let on_a = Image::along(curve.clone(), *charts[0])?;
    let on_b = Image::along(curve.clone(), *charts[1])?;
    Some(SectionCurve { curve: Arc::new(curve), on_a: Arc::new(on_a), on_b: Arc::new(on_b) })
}

/// The pieces of `curve`, cut where an image is undefined, each `trim`med from it and imaged.
fn pieces<C: ParametricCurve + Clone, T: ParametricCurve + Clone>(
    curve: &C,
    trim: impl Fn(f64, f64) -> Option<T>,
    (points, charts): (&[Vec3], [&Chart; 2]),
    decide: &mut Decisions,
) -> Result<Option<Vec<SectionCurve>>, Undecided> {
    let split = parts(Curve::range(curve), &cuts(curve, points, decide)?);
    if split.is_empty() {
        return Ok(imaged(curve.clone(), charts).map(|c| vec![c]));
    }
    Ok(split.into_iter().map(|(lo, hi)| imaged(trim(lo, hi)?, charts)).collect())
}

/// A contact at `at`, with its parameters on both surfaces.
fn contact(at: Vec3, charts: [&Chart; 2]) -> Contact {
    let p = Point::from_vec3(at);
    Contact::Point { at: p, on_a: charts[0].parameters(p), on_b: charts[1].parameters(p) }
}

/// Lines and conics, made and imaged by the closed forms.
fn conics(
    found: Result<Found, &'static str>,
    charts: [&Chart; 2],
    decide: &mut Decisions,
    section: &mut Section,
) -> Result<Result<(), &'static str>, Undecided> {
    let found = match found {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    Ok(if emit(found, charts, decide, section)? { Ok(()) } else { Err("a conic with no image") })
}

/// The curves a class is made of, or why it has none here.
type Made = Result<Result<Vec<SectionCurve>, &'static str>, Undecided>;

/// Where an image is undefined, and the charts the images are read through.
type Charts<'a> = (&'a [Vec3], [&'a Chart; 2]);

/// The curves of `fan` over `stretches`, each a B-spline cut where an image is undefined and imaged.
fn fanned(pair: &Pair, fan: &Fan, stretches: Vec<(f64, f64)>, charts: Charts<'_>, decide: &mut Decisions) -> Made {
    let mut out = Vec::new();
    for stretch in stretches {
        let Some(c) = fan.spline(pair, stretch) else { return Ok(Err("a rational curve with no B-spline")) };
        let Some(made) = pieces(&c, |lo, hi| TrimmedCurve::new(c.clone(), lo, hi).ok(), charts, decide)? else {
            return Ok(Err("a rational curve with no image"));
        };
        out.extend(made);
    }
    Ok(Ok(out))
}

/// A smooth quartic's loops, each whole, or, where it reaches infinity, within `ball`.
fn quartics(
    pair: &Pair,
    (ruling, o): (&Ruling, usize),
    ball: Option<Ball>,
    charts: Charts<'_>,
    decide: &mut Decisions,
) -> Made {
    let along = ruling.along(&pair.quadric(o));
    let Some(loops) = smooth::loops(pair, ruling, &along) else { return Ok(Err("a loop with no form")) };
    let mut out = Vec::new();
    for whole in &loops {
        let parts = ball.map_or_else(|| vec![(0.0, TAU)], |b| smooth::within(whole, (ruling, &along), pair.origin, b));
        for (lo, hi) in parts {
            let part = if hi - lo >= TAU { Some(whole.quartic) } else { whole.quartic.trimmed(lo, hi).ok() };
            let Some(q) = part else { return Ok(Err("a loop with no part")) };
            let Some(made) = pieces(&q, |lo, hi| q.trimmed(lo, hi).ok(), charts, decide)? else {
                return Ok(Err("a loop with no image"));
            };
            out.extend(made);
        }
    }
    Ok(Ok(out))
}

/// A nodal quartic, its node a contact: round the whole turn from the node, or, where it reaches infinity, within
/// `ball`.
fn nodal(
    pair: &Pair,
    (phi, at): (f64, Vec3),
    ball: Ball,
    charts: Charts<'_>,
    decide: &mut Decisions,
    section: &mut Section,
) -> Made {
    let nodal = nodal::node(pair, at, phi, decide)?;
    let made = match &nodal.fan {
        None => Vec::new(),
        Some(fan) => {
            let stretches = if nodal.reaching { fan.within(ball) } else { vec![(nodal.start, nodal.start + TAU)] };
            match fanned(pair, fan, stretches, charts, decide)? {
                Ok(made) => made,
                Err(why) => return Ok(Err(why)),
            }
        }
    };
    section.contacts.push(contact(nodal.at, charts.1));
    Ok(Ok(made))
}

/// A quartic with its node at infinity, along the axis of `ruling`, the pair's cylinder: one point on each of its
/// rulings, within `ball`.
fn at_infinity(
    pair: &Pair,
    (ruling, o): (&Ruling, usize),
    ball: Ball,
    charts: Charts<'_>,
    decide: &mut Decisions,
) -> Made {
    let Kind::Upright { radius } = ruling.kind else { return Ok(Err("a node at infinity with no cylinder")) };
    let along = ruling.along(&pair.quadric(o));
    let fan = Fan::along([ruling.origin, ruling.x * radius, ruling.y * radius], ruling.z, (&along.beta, &along.gamma));
    fanned(pair, &fan, fan.within(ball), charts, decide)
}

/// The ruling of the pair's cone, surface `o`, through its apex along the axis of `ruling`, the pair's cylinder, which
/// it crosses there, and the cubic: one point on each of the cone's rulings, within `ball`.
fn ruling_and_cubic(
    pair: &Pair,
    (ruling, o): (&Ruling, usize),
    ball: Ball,
    (charts, decide): (Charts<'_>, &mut Decisions),
    section: &mut Section,
) -> Made {
    let (Some(cone), Natural::Cone { apex, .. }) = (Ruling::of(&pair.surface(o), pair.origin), pair.surface(o)) else {
        return Ok(Err("a ruling and a cubic with no cone"));
    };
    let Kind::Slanted { sin, cos } = cone.kind else { return Ok(Err("a ruling and a cubic with no cone")) };
    let along = cone.along(&pair.quadric(1_usize.saturating_sub(o)));
    let fan = Fan::through(cone.origin, [cone.z * cos, cone.x * sin, cone.y * sin], (&along.alpha, &along.beta));
    let cubic = match fanned(pair, &fan, fan.within(ball), charts, decide)? {
        Ok(cubic) => cubic,
        Err(why) => return Ok(Err(why)),
    };
    let line = Found { curves: vec![Plan::Ruling { apex, along: ruling.z }], contacts: vec![apex], ..Found::default() };
    Ok(conics(Ok(line), charts.1, decide, section)?.map(|()| cubic))
}

/// Puts what `class` is into `section`, a section that reaches infinity within `ball`: `Ok(Err)`, with nothing put,
/// where this method does not parametrize it.
pub(super) fn built(
    pair: &Pair,
    class: Class,
    (charts, ball): ([&Chart; 2], Ball),
    decide: &mut Decisions,
    section: &mut Section,
) -> Result<Result<(), &'static str>, Undecided> {
    let points = undefined(pair, charts);
    let at = (points.as_slice(), charts);
    let Some((_, o, ruling)) = pair.ruled() else { return Ok(Err("no ruled member")) };
    let made = match class {
        Class::Smooth => quartics(pair, (&ruling, o), None, at, decide)?,
        Class::Reaching => quartics(pair, (&ruling, o), Some(ball), at, decide)?,
        Class::Node { phi, at: node, .. } => nodal(pair, (phi, node), ball, at, decide, section)?,
        Class::AtInfinity => at_infinity(pair, (&ruling, o), ball, at, decide)?,
        Class::RulingAndCubic => ruling_and_cubic(pair, (&ruling, o), ball, (at, decide), section)?,
        Class::Split { planes } => return conics(split::conics(pair, planes, decide)?, charts, decide, section),
        Class::Lines => return conics(split::rulings(pair, decide)?, charts, decide, section),
        Class::Conjugate { line } => {
            let points = match line {
                Some(planes) => split::touches(pair, planes, decide)?,
                None => Vec::new(),
            };
            section.contacts.extend(points.into_iter().map(|at| contact(at, charts)));
            Ok(Vec::new())
        }
        Class::Other(why) => Err(why),
        Class::Parallel => Ok(Vec::new()),
    };
    Ok(made.map(|curves| section.curves.extend(curves)))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use occam_geom::{Line, ParamRange};
    use occam_units::{Direction, ParamKind};

    use super::*;

    /// Where the line along X through the origin comes nearest `(x, 1, 0)` from `lo` to `hi`.
    fn on_x(x: f64, lo: f64, hi: f64) -> Vec<(f64, u32)> {
        let line =
            Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        nearest(&line, Vec3::new(x, 1.0, 0.0), (lo, hi))
    }

    #[test]
    fn a_curve_comes_nearest_a_point_where_the_slope_of_its_distance_rises_through_zero_in_its_range() {
        let once = |got: &[(f64, u32)], t: f64, sample: u32| {
            got.len() == 1 && got.iter().all(|(at, i)| (at - t).abs() <= 1e-15 && *i == sample)
        };
        let got = on_x(0.3, 0.0, 1.0);
        assert!(once(&got, 0.3, 19), "between samples: {got:?}");
        let got = on_x(0.5, 0.0, 1.0);
        assert!(once(&got, 0.5, 31), "at a sample, found once: {got:?}");
        let got = on_x(2.5, 1.0, 3.0);
        assert!(once(&got, 2.5, 47), "in a range from 1: {got:?}");
        assert_eq!(on_x(4.0, 1.0, 3.0), Vec::new(), "beyond its end");
    }

    #[test]
    fn a_range_is_cut_into_parts_from_cut_to_cut_round_its_period() {
        let round = ParamRange { lo: 1.0, hi: 1.0 + TAU, wrap: Wrap::Periodic, kind: ParamKind::Angle };
        assert_eq!(parts(round, &[2.0, 3.0]), vec![(2.0, 3.0), (3.0, 2.0 + TAU)]);
        assert_eq!(parts(round, &[2.0]), vec![(2.0, 2.0 + TAU)]);
        assert_eq!(parts(round, &[]), Vec::new());
        let open = ParamRange { lo: 1.0, hi: 5.0, wrap: Wrap::Open, kind: ParamKind::Knot };
        assert_eq!(parts(open, &[2.0, 3.0]), vec![(1.0, 2.0), (2.0, 3.0), (3.0, 5.0)]);
        assert_eq!(parts(open, &[]), vec![(1.0, 5.0)]);
    }
}
