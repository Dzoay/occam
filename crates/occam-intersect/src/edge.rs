//! Source: original
//! Where a trace meets an edge of a surface's domain (P5). A parameter that comes round has no edge. At one that stops,
//! the edge is a seam where the surface's point at the other end is the same, as at a closed NURBS's first and last
//! knots, and the curve carries on from there; a pole where the line of the domain is declared to collapse to a point,
//! as at a sphere's, and the curve carries on through it; and otherwise the end of the surface, where the curve leaves
//! it. A contact at a pole is not the curve's end. Each of these is decided against the band, through [`Decide`]: a
//! parameter's distance past the nearer end of its range, in the length it moves the point, the edge's distance from
//! a declared pole, and the distance between the surface's points at the edge and at the other end.

use occam_geom::{Param, ParamRange, Scratch, Wrap};
use occam_math::Side as Band;
use occam_units::{Point, UvPoint};

use crate::correct::{Here, Pair, Side};
use crate::decide::{Decide, distance, past};
use crate::trace::{Jump, Uncertain};

/// A parameter of one side on or past an end of its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Breach {
    /// The side.
    pub side: Side,
    /// The parameter.
    pub param: Param,
    /// The end it is on or past.
    pub end: f64,
    /// The other end.
    pub other: f64,
    /// Whether it is on the end, within the band of it, rather than past it.
    pub on: bool,
}

/// What the curve does at an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Meets {
    /// It carries on across the edge.
    Across(Jump),
    /// It leaves the surface.
    Leaves,
}

/// `uv`'s value of `param`.
pub(crate) fn value(uv: UvPoint, param: Param) -> f64 {
    match param {
        Param::U => uv.u(),
        Param::V => uv.v(),
    }
}

/// `uv` with its value of `param` set to `to`.
pub(crate) fn with(uv: UvPoint, param: Param, to: f64) -> UvPoint {
    match param {
        Param::U => UvPoint::new(to, uv.v()),
        Param::V => UvPoint::new(uv.u(), to),
    }
}

/// The parameters of `side` at `here`.
pub(crate) const fn on(here: &Here, side: Side) -> UvPoint {
    match side {
        Side::P => here.p,
        Side::Q => here.q,
    }
}

/// `here` with the parameters of `side` set to `uv`.
pub(crate) const fn moved(here: Here, side: Side, uv: UvPoint) -> Here {
    match side {
        Side::P => Here { p: uv, ..here },
        Side::Q => Here { q: uv, ..here },
    }
}

/// Where a parameter at `t` lies against the nearer end of `range`, which stops at its ends, moving the point at `here`
/// by `rate` a unit: decided on how far it is past that end, in length.
fn against(
    decide: &mut Decide,
    (range, t, rate): (ParamRange, f64, f64),
    (side, param): (Side, Param),
    here: &Here,
) -> Result<Option<Breach>, Uncertain> {
    if range.wrap == Wrap::Periodic || !range.lo.is_finite() || !range.hi.is_finite() {
        return Ok(None);
    }
    let upward = (t - range.lo).abs() >= (range.hi - t).abs();
    let (end, other) = if upward { (range.hi, range.lo) } else { (range.lo, range.hi) };
    let breach = |on: bool| Some(Breach { side, param, end, other, on });
    Ok(match decide.side(past(t, end, rate, upward), here.at)? {
        Band::Below => None,
        Band::Within => breach(true),
        Band::Above => breach(false),
    })
}

/// Each of `side`'s parameters at `here` that is on or past the nearer end of its range.
fn edges(pair: &Pair<'_>, decide: &mut Decide, here: &Here, side: Side) -> Result<[Option<Breach>; 2], Uncertain> {
    let (surface, uv) = (pair.surface(side), on(here, side));
    let domain = surface.domain();
    let stops = |range: ParamRange| range.wrap != Wrap::Periodic && range.lo.is_finite();
    if !stops(domain.u) && !stops(domain.v) {
        return Ok([None, None]);
    }
    let jet = surface.first(uv);
    let u = against(decide, (domain.u, uv.u(), jet.du.rate().norm()), (side, Param::U), here)?;
    let v = against(decide, (domain.v, uv.v(), jet.dv.rate().norm()), (side, Param::V), here)?;
    Ok([u, v])
}

/// The first parameter of `here`, `p`'s then `q`'s, on or past an end of its range.
pub(crate) fn breach(pair: &Pair<'_>, decide: &mut Decide, here: &Here) -> Result<Option<Breach>, Uncertain> {
    for side in [Side::P, Side::Q] {
        if let Some(found) = edges(pair, decide, here, side)?.into_iter().flatten().next() {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// Where `here` lies against the end of its range that `edge` names: below the band inside it, within it on the
/// end, and above it past the end.
pub(crate) fn beside(pair: &Pair<'_>, decide: &mut Decide, here: &Here, edge: &Breach) -> Result<Band, Uncertain> {
    let uv = on(here, edge.side);
    let jet = pair.surface(edge.side).first(uv);
    let rate = if edge.param == Param::U { jet.du.rate().norm() } else { jet.dv.rate().norm() };
    decide.side(past(value(uv, edge.param), edge.end, rate, edge.end > edge.other), here.at)
}

/// What the curve does at the edge `breach` names, met at `edge`: a pole where the edge is within the band of a point
/// the surface declares its line of `breach`'s parameter to collapse to, a seam where the surface's point at the
/// other end is within the band of its point at this one, and otherwise the surface's end.
pub(crate) fn meets(pair: &Pair<'_>, decide: &mut Decide, edge: &Here, breach: &Breach) -> Result<Meets, Uncertain> {
    let surface = pair.surface(breach.side);
    for line in surface.degeneracies().iter().filter(|line| line.param == breach.param) {
        if decide.side(distance(edge.at, line.point.to_vec3()), edge.at)? == Band::Within {
            return Ok(Meets::Across(Jump::Pole));
        }
    }
    let uv = on(edge, breach.side);
    let there = surface.point(with(uv, breach.param, breach.other)).to_vec3();
    let here = surface.point(with(uv, breach.param, breach.end)).to_vec3();
    Ok(if decide.side(distance(there, here), edge.at)? == Band::Within {
        Meets::Across(Jump::Seam)
    } else {
        Meets::Leaves
    })
}

/// `here` put inside `side`'s domain: each parameter on an end put on it, and where one is past an end, the side's
/// parameters those of its closest point, once. `None` where it cannot be.
fn into(pair: &Pair<'_>, decide: &mut Decide, here: Here, side: Side, again: bool) -> Result<Option<Here>, Uncertain> {
    let found = edges(pair, decide, &here, side)?;
    if found.iter().flatten().any(|breach| !breach.on) {
        let surface = pair.surface(side);
        let point = Point::from_vec3(here.at);
        let closest = surface.closest(point, Some(on(&here, side)), pair.tol, &mut Scratch::default());
        let Some(closest) = closest.ok().filter(|closest| again && closest.distance <= pair.tol.band()) else {
            return Ok(None);
        };
        return into(pair, decide, moved(here, side, closest.at), side, false);
    }
    let put = |here: Here, breach: &Breach| moved(here, side, with(on(&here, side), breach.param, breach.end));
    Ok(Some(found.iter().flatten().fold(here, put)))
}

/// `here` put inside both domains: how a start or a touch, which no trace reached the edge of, is. `None` where it
/// cannot be.
pub(crate) fn inside(pair: &Pair<'_>, decide: &mut Decide, here: Here) -> Result<Option<Here>, Uncertain> {
    let Some(here) = into(pair, decide, here, Side::P, true)? else { return Ok(None) };
    into(pair, decide, here, Side::Q, true)
}
