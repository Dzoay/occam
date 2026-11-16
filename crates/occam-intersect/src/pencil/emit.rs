//! Source: original
//! What the pencil method puts in the section: each class's curves, made, with their images on both surfaces, and its
//! singular points as contacts. Lines and conics are made and imaged by the closed forms' own code. A quartic's loop and
//! a nodal quartic's B-spline are read through each surface's chart by occam-geom's image family, counting the turns
//! they make about the chart's axis, and each is first cut where it passes a point at which a chart's parameters are
//! undefined: a sphere's pole or a cone's apex.

use std::sync::Arc;

use occam_geom::{Chart, Curve, Image, ParametricCurve, Scratch, TrimmedCurve, Wrap};
use occam_math::{Interval, Side};
use occam_units::{Point, Vec3};

use super::quadric::Natural;
use super::{Class, Pair, nodal, smooth, split};
use crate::closed::{Found, emit};
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

/// The parameters at which `curve` passes through one of `points`, within the band.
fn cuts(curve: &dyn Curve, points: &[Vec3], decide: &mut Decisions) -> Result<Vec<f64>, Undecided> {
    let mut out = Vec::new();
    for p in points {
        let Ok(near) = curve.closest(Point::from_vec3(*p), None, decide.tolerance(), &mut Scratch::default()) else {
            continue;
        };
        let distance = Interval::new(near.distance.millimetres(), 4.0 * f64::EPSILON * (1.0 + p.norm()));
        if decide.length(distance)? == Side::Within {
            out.push(near.at);
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
    Ok(out)
}

/// The parts of a range cut at `cuts`, each from one cut to the next, round the period where it comes round.
fn parts(range: occam_geom::ParamRange, cuts: &[f64]) -> Vec<(f64, f64)> {
    let inside: Vec<f64> = cuts.iter().copied().filter(|t| *t > range.lo + 1e-12 && *t < range.hi - 1e-12).collect();
    if range.wrap == Wrap::Periodic {
        let (Some(first), Some(last)) = (inside.first().copied(), inside.last().copied()) else { return Vec::new() };
        let mut out: Vec<(f64, f64)> = inside.windows(2).filter_map(|w| Some((*w.first()?, *w.last()?))).collect();
        out.push((last, first + (range.hi - range.lo)));
        return out;
    }
    let ends: Vec<f64> = std::iter::once(range.lo).chain(inside).chain([range.hi]).collect();
    ends.windows(2).filter_map(|w| Some((*w.first()?, *w.last()?))).collect()
}

/// `curve` with its images through both charts, or `None` where one cannot be read.
fn imaged<C: ParametricCurve + Clone>(curve: C, charts: [&Chart; 2]) -> Option<SectionCurve> {
    let on_a = Image::along(curve.clone(), *charts[0])?;
    let on_b = Image::along(curve.clone(), *charts[1])?;
    Some(SectionCurve { curve: Arc::new(curve), on_a: Arc::new(on_a), on_b: Arc::new(on_b) })
}

/// The pieces of `curve`, cut where an image is undefined, each `trim`med from it and imaged. A cut where an open
/// curve already ends is no cut: a curve at rest there, as at a cusp, finds its foot only to the square root of the
/// rounding in its parameter.
fn pieces<C: ParametricCurve + Clone, T: ParametricCurve + Clone>(
    curve: &C,
    trim: impl Fn(f64, f64) -> Option<T>,
    (points, charts): (&[Vec3], [&Chart; 2]),
    decide: &mut Decisions,
) -> Result<Option<Vec<SectionCurve>>, Undecided> {
    let range = Curve::range(curve);
    let ends = [range.lo, range.hi].map(|t| curve.point(t).to_vec3());
    let at: Vec<f64> = cuts(curve, points, decide)?
        .into_iter()
        .filter(|t| {
            let p = curve.point(*t).to_vec3();
            range.wrap == Wrap::Periodic || ends.iter().all(|e| (*e - p).norm() > 1e-9 * (1.0 + p.norm()))
        })
        .collect();
    let split = parts(range, &at);
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

/// Puts what `class` is into `section`: `Ok(Err)`, with nothing put, where this method does not parametrize it.
pub(super) fn built(
    pair: &Pair,
    class: Class,
    charts: [&Chart; 2],
    decide: &mut Decisions,
    section: &mut Section,
) -> Result<Result<(), &'static str>, Undecided> {
    let points = undefined(pair, charts);
    let Some((_, o, ruling)) = pair.ruled() else { return Ok(Err("no ruled member")) };
    let curves = match class {
        Class::Smooth => {
            let along = ruling.along(&pair.quadric(o));
            let Some(loops) = smooth::loops(pair, &ruling, &along) else { return Ok(Err("a loop with no form")) };
            let mut out = Vec::new();
            for q in loops {
                let Some(made) = pieces(&q, |lo, hi| q.trimmed(lo, hi).ok(), (&points, charts), decide)? else {
                    return Ok(Err("a loop with no image"));
                };
                out.extend(made);
            }
            out
        }
        Class::Node { phi, at, .. } => {
            let nodal = match nodal::node(pair, at, phi) {
                Ok(nodal) => nodal,
                Err(why) => return Ok(Err(why)),
            };
            let made = match nodal.curve {
                None => Some(Vec::new()),
                Some(c) => pieces(&c, |lo, hi| TrimmedCurve::new(c.clone(), lo, hi).ok(), (&points, charts), decide)?,
            };
            let Some(made) = made else { return Ok(Err("a nodal quartic with no image")) };
            section.contacts.push(contact(nodal.at, charts));
            made
        }
        Class::Split { planes } => return conics(split::conics(pair, planes, decide)?, charts, decide, section),
        Class::Lines => return conics(split::rulings(pair, decide)?, charts, decide, section),
        Class::Other(why) => return Ok(Err(why)),
        Class::Parallel => Vec::new(),
    };
    section.curves.extend(curves);
    Ok(Ok(()))
}
