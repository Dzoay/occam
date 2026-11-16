//! Source: original
//! The hybrid near a tangency (O24, P5). Newton's method loses rank where two surfaces nearly touch, as their normals
//! are nearly parallel there, so a leaf whose middle it settles from nowhere is given to alternating projections, which
//! do not: where they come to rest inside the leaf on both surfaces, that is its start point. Near a shallow crossing a
//! second curve may pass through the same leaf, which gives one start point only: the gap between the surfaces is near
//! a quadratic there, even about where it is least or most, so the point mirrored through that place is on the second
//! curve where there is one, and is a start point too, traced after the leaves' own.
//!
//! A trace that ends in a touch, alone or at the end of a curve, as at a branch point, gives a contact there by
//! [`resolution`](crate::resolution)'s rule: the bound of what may hide there, from the gap, its slope and the relative
//! curvature at the touch, decided against the band, a point only where that is within it. Contacts whose places lie
//! within each other's bounds are one, its bound grown to hold both, so that the touches along a tangency, or about a
//! branch point from each of its branches, are one contact.

use occam_geom::{Domain, Surface, UvBox};
use occam_math::{Interval, Side, Tally};
use occam_units::{UvPoint, Vec3};

use crate::correct::{Here, Pair, solve};
use crate::decide::{Decide, distance};
use crate::project::project;
use crate::resolution::{Near, contact};
use crate::section::Contact;
use crate::trace::{End, Start, Station, Trace, Uncertain};

/// A start point and where it is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Seed {
    pub start: Start,
    pub at: Vec3,
}

/// The seed of `here` on `pair`.
pub(crate) fn seed_of(pair: &Pair<'_>, here: &Here) -> Seed {
    let station = pair.station(here);
    Seed { start: Start { on_a: station.on_a, on_b: station.on_b }, at: here.at }
}

/// What a leaf's middle gave beside its start point: the start of a second curve through it, the decisions made in
/// finding a start by projections, and one the band could not make.
#[derive(Default)]
pub(crate) struct Beside {
    pub twin: Option<Seed>,
    pub tally: Tally,
    pub uncertain: Option<Uncertain>,
}

/// The two surfaces of `pair` in the order they were given.
fn given<'s>(pair: &Pair<'s>) -> (&'s dyn Surface, &'s dyn Surface) {
    if pair.swapped { (pair.q, pair.p) } else { (pair.p, pair.q) }
}

/// The parameters of `surface` a move of `by` from its point at `uv` leads to, to first order.
fn nudged(surface: &dyn Surface, uv: UvPoint, by: Vec3) -> UvPoint {
    let jet = surface.first(uv);
    let (du, dv) = (jet.du.rate(), jet.dv.rate());
    let normal = [[du.dot(du), du.dot(dv)], [du.dot(dv), dv.dot(dv)]];
    solve(normal, [du.dot(by), dv.dot(by)]).map_or(uv, |[u, v]| UvPoint::new(uv.u() + u, uv.v() + v))
}

/// The start of a second curve near `here`, a start point on `pair` of a leaf `reach` across: the point mirrored
/// through where the gap is least or most, settled onto both surfaces, where that is within `reach` and more than the
/// band away.
pub(crate) fn twin(pair: &Pair<'_>, here: &Here, reach: f64) -> Option<Seed> {
    let (a, b) = given(pair);
    let station = pair.station(here);
    let gap = Near::between(a, station.on_a, b, station.on_b)?;
    let [x, y] = gap.critical();
    let by = gap.lifted([2.0 * x, 2.0 * y]);
    let far = by.norm();
    let beside = far > pair.band() && far <= reach;
    if !beside {
        return None;
    }
    // Settling reads the parameters alone, and finds the point from them.
    let moved = Here { p: nudged(pair.p, here.p, by), q: nudged(pair.q, here.q, by), ..*here };
    pair.settle(&moved).map(|found| seed_of(pair, &found))
}

/// Whether `uv` is inside `patch`, `hair` of its sides past its edges included, and inside `domain`.
pub(crate) fn inside(patch: UvBox, uv: UvPoint, domain: Domain, hair: f64) -> bool {
    let near = |x: f64, lo: f64, hi: f64| {
        let past = (hi - lo) * hair;
        lo - past <= x && x <= hi + past
    };
    near(uv.u(), patch.lo.u(), patch.hi.u()) && near(uv.v(), patch.lo.v(), patch.hi.v()) && domain.contains(uv)
}

/// The start alternating projections from `seed` come to rest on, where that is on both surfaces of `pair` inside the
/// leaf of `p`'s part and, if given, `q`'s, with `hair` of their sides past their edges: where Newton's method from the
/// leaf's middle found none.
pub(crate) fn projected(
    pair: &Pair<'_>,
    seed: &Here,
    (p, q): (UvBox, Option<UvBox>),
    hair: f64,
) -> (Option<Seed>, Beside) {
    let mut decide = Decide::new(pair.band());
    let (found, _) = project(pair, &mut decide, seed);
    let mut beside = Beside::default();
    let found = match found {
        Ok(found) => found,
        Err(open) => {
            beside.uncertain = Some(open);
            None
        }
    };
    beside.tally = decide.tally;
    let ours =
        |h: &Here| inside(p, h.p, pair.p.domain(), hair) && q.is_none_or(|q| inside(q, h.q, pair.q.domain(), hair));
    (found.filter(ours).map(|here| seed_of(pair, &here)), beside)
}

/// A contact found, before it is decided: where, its parameters on both surfaces, and its bound.
#[derive(Clone, Copy, Debug)]
struct Found {
    at: Vec3,
    on: (UvPoint, UvPoint),
    bound: f64,
}

/// The touches `kept` ended in, each a station of a trace: a trace that is a touch alone, and each end of the others
/// that is one.
fn touched(kept: &[Trace]) -> Vec<Station> {
    let mut out = Vec::new();
    for trace in kept {
        if let ([station], [End::Touch, End::Touch]) = (trace.stations.as_slice(), trace.ends) {
            out.push(*station);
            continue;
        }
        let ends = [(trace.ends[0], trace.stations.first()), (trace.ends[1], trace.stations.last())];
        out.extend(ends.into_iter().filter(|(end, _)| *end == End::Touch).filter_map(|(_, station)| station.copied()));
    }
    out
}

/// Whether `at` lies within `bound` of `from`, decided against the band: on its edge is within.
fn holds(from: Vec3, bound: f64, at: Vec3, decide: &mut Decide) -> Result<bool, Uncertain> {
    if bound == f64::INFINITY {
        return Ok(true);
    }
    Ok(decide.side(distance(from, at) - Interval::exact(bound), at)? != Side::Above)
}

/// The contacts of the branch points `branches` and of the touches `kept` ended in, between `a` and `b`, by the rule:
/// each bound from the gap there,
/// those within each other's bounds made one, and each a point only where its bound is decided within the band. A
/// decision the band cannot make is given back in `uncertain`, and the contact is below the resolution.
pub(crate) fn contacts(
    (a, b): (&dyn Surface, &dyn Surface),
    (kept, branches): (&[Trace], &[Station]),
    decide: &mut Decide,
    uncertain: &mut Vec<Uncertain>,
    band: f64,
) -> Vec<Contact> {
    let mut found: Vec<Found> = Vec::new();
    for station in branches.iter().copied().chain(touched(kept)) {
        let gap = Near::between(a, station.on_a, b, station.on_b);
        let bound = gap.map_or(f64::INFINITY, |gap| gap.bound(band));
        let here = Found { at: station.at.to_vec3(), on: (station.on_a, station.on_b), bound };
        let mut merged = false;
        for other in &mut found {
            match holds(other.at, other.bound, here.at, decide) {
                Ok(true) => {
                    let reach = (here.at - other.at).norm() + here.bound;
                    other.bound = if reach > other.bound { reach } else { other.bound };
                    merged = true;
                    break;
                }
                Ok(false) => {}
                Err(open) => uncertain.push(open),
            }
        }
        if !merged {
            found.push(here);
        }
    }
    found.into_iter().map(|f| decided(f, decide, uncertain)).collect()
}

/// `found` as a contact: a point where its bound is decided within the band. An infinite bound is not.
fn decided(found: Found, decide: &mut Decide, uncertain: &mut Vec<Uncertain>) -> Contact {
    let certified = found.bound.is_finite()
        && match decide.side(Interval::exact(found.bound), found.at) {
            Ok(side) => side == Side::Within,
            Err(open) => {
                uncertain.push(open);
                false
            }
        };
    contact(found.at, found.on, found.bound, certified)
}

#[cfg(test)]
mod tests {
    use occam_geom::Cylinder;
    use occam_math::Tally;
    use occam_units::Point;

    use super::*;
    use crate::trace_tests::{BAND, ball, level, mm, moved, placed, tol, upright};

    /// The station at `at` on `a` and `b`, its parameters each's inverse of it.
    fn station(a: &dyn Surface, b: &dyn Surface, at: Vec3) -> Station {
        let (point, mut scratch) = (Point::from_vec3(at), occam_geom::Scratch::default());
        let on_a = a.invert(point, None, tol(), &mut scratch).unwrap();
        Station { at: point, on_a, on_b: b.invert(point, None, tol(), &mut scratch).unwrap() }
    }

    /// A trace of `stations` ending as `ends`.
    fn trace(stations: Vec<Station>, ends: [End; 2]) -> Trace {
        Trace { stations, crossings: vec![], ends, steps: 0, tally: Tally::default(), undecided: vec![] }
    }

    #[test]
    fn the_second_of_two_lines_close_together_starts_where_the_first_is_mirrored_through_their_ridge() {
        // The plane x = 1 - 1e-4 inside the unit cylinder about Z: two lines at y = ±√(2e-4 - 1e-8).
        let (plane, tube) = (upright(1.0 - 1e-4), Cylinder::placed(&moved([0.0; 3]), mm(1.0)).unwrap());
        let pair = Pair::new(&plane, &tube, tol());
        let y = (2e-4_f64 - 1e-8).sqrt();
        let first = station(&plane, &tube, Vec3::new(1.0 - 1e-4, y, 0.3));
        let here = pair.settle(&pair.seed(Start { on_a: first.on_a, on_b: first.on_b })).unwrap();
        let other = twin(&pair, &here, 0.1).unwrap();
        let want = Vec3::new(1.0 - 1e-4, -y, 0.3);
        assert!((other.at - want).norm() <= 1e-6, "{:?} against {want:?}", other.at);
        assert!(twin(&pair, &here, 0.02).is_none(), "beyond the leaf's reach");
    }

    #[test]
    fn no_second_curve_starts_within_the_band_of_the_first() {
        // At the touch of a sphere on a plane, where the gap is least, the mirror is the touch itself.
        let (plane, sphere) = (level(0.0), ball(2.0, [0.0, 0.0, 2.0]));
        let pair = Pair::new(&plane, &sphere, tol());
        let touch = Here { at: Vec3::new(0.0, 0.0, 0.0), p: UvPoint::new(0.0, 0.0), q: UvPoint::new(0.0, -1.5) };
        let touch = Here { q: pair.q_at(touch.at, touch.q).unwrap(), ..touch };
        assert!(twin(&pair, &touch, 1.0).is_none());
    }

    #[test]
    fn projections_find_a_touch_inside_the_leaf_newtons_method_cannot() {
        let (plane, sphere) = (level(0.0), ball(2.0, [0.0, 0.0, 2.0]));
        let pair = Pair::new(&plane, &sphere, tol());
        let from = Here { at: Vec3::new(0.3, 0.2, 0.0), p: UvPoint::new(0.3, 0.2), q: UvPoint::new(0.0, 0.0) };
        let whole = UvBox::new(UvPoint::new(-1.0, -1.0), UvPoint::new(1.0, 1.0));
        let (seed, beside) = projected(&pair, &from, (whole, None), 1.0 / 1024.0);
        let seed = seed.unwrap();
        assert!(seed.at.norm() <= 1e-3, "{:?}", seed.at);
        assert!(beside.tally.decided > 0 && beside.uncertain.is_none() && beside.twin.is_none(), "{:?}", beside.tally);
        let aside = UvBox::new(UvPoint::new(0.5, 0.5), UvPoint::new(1.0, 1.0));
        assert!(projected(&pair, &from, (aside, None), 1.0 / 1024.0).0.is_none(), "not this leaf's");
        let (high, far) = (level(3.0), Here { at: Vec3::new(0.0, 0.0, 3.0), ..from });
        let apart = Pair::new(&high, &sphere, tol());
        assert!(projected(&apart, &far, (whole, None), 1.0 / 1024.0).0.is_none(), "apart");
    }

    #[test]
    fn a_touch_is_a_trace_alone_or_an_end_of_one_that_is_a_touch() {
        let s = |x: f64| Station {
            at: Point::from_vec3(Vec3::new(x, 0.0, 0.0)),
            on_a: UvPoint::new(x, 0.0),
            on_b: UvPoint::new(x, 0.0),
        };
        let kept = [
            trace(vec![s(0.0)], [End::Touch; 2]),
            trace(vec![s(1.0), s(1.5)], [End::Touch, End::Edge]),
            trace(vec![s(2.0), s(2.5)], [End::Edge, End::Touch]),
            trace(vec![s(3.0), s(3.5), s(3.7)], [End::Closed; 2]),
            trace(vec![s(6.0), s(6.5), s(6.7)], [End::Edge, End::Touch]),
            trace(vec![s(4.0)], [End::Apart; 2]),
            trace(vec![s(5.0)], [End::Undecided; 2]),
            trace(vec![], [End::Touch; 2]),
        ];
        let got: Vec<f64> = touched(&kept).iter().map(|s| s.at.to_vec3().x).collect();
        assert_eq!(got, [0.0, 1.0, 2.5, 6.7]);
    }

    #[test]
    fn touches_within_each_others_bounds_are_one_contact_and_those_apart_are_two() {
        // A sphere of radius 2 touching the plane z = 0 at the origin: each touch near it is bounded by about
        // 2 √(2 band / ½), and two 1e-5 apart are one, its bound grown to hold the second's, 1e-5 off its middle: by
        // 1e-5 and twice that. One 1.5 away is another.
        let (plane, sphere) = (level(0.0), ball(2.0, [0.0, 0.0, 2.0]));
        let near = |x: f64| station(&plane, &sphere, Vec3::new(x, 0.0, 2.0 - (4.0 - x * x).sqrt()));
        let kept = [
            trace(vec![near(0.0)], [End::Touch; 2]),
            trace(vec![near(1e-5)], [End::Touch; 2]),
            trace(vec![near(1.5)], [End::Touch; 2]),
        ];
        let (mut decide, mut open) = (Decide::new(BAND), Vec::new());
        let got = contacts((&plane, &sphere), (&kept, &[]), &mut decide, &mut open, BAND);
        assert!(open.is_empty() && decide.tally.decided >= 3, "{open:?}");
        let [Contact::BelowResolution { at, bound }, Contact::BelowResolution { at: other, .. }] = got.as_slice()
        else {
            panic!("{got:?}")
        };
        let d = (2.0 * BAND / 0.5).sqrt();
        assert!(at.to_vec3().norm() <= 1e-12 && (bound.millimetres() - (2.0 * d + 3e-5)).abs() <= 1e-6, "{bound:?}");
        assert!((other.to_vec3().x - 1.5).abs() <= 1e-9, "{other:?}");
    }

    #[test]
    fn a_touch_on_a_tangency_along_a_curve_has_no_finite_bound_and_holds_every_other() {
        // The unit cylinder along X resting on the plane z = 0: tangent along the X axis.
        let plane = level(0.0);
        let tube =
            Cylinder::placed(&placed([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2, [0.0, 0.0, 1.0]), mm(1.0)).unwrap();
        let on = |x: f64| station(&plane, &tube, Vec3::new(x, 0.0, 0.0));
        let kept = [trace(vec![on(0.0)], [End::Touch; 2]), trace(vec![on(3.0)], [End::Touch; 2])];
        let (mut decide, mut open) = (Decide::new(BAND), Vec::new());
        let got = contacts((&plane, &tube), (&kept, &[on(-2.0)]), &mut decide, &mut open, BAND);
        let [Contact::BelowResolution { at, bound }] = got.as_slice() else { panic!("{got:?}") };
        assert_eq!((at.to_vec3().x, bound.millimetres(), decide.tally.decided), (-2.0, f64::INFINITY, 0));
    }

    #[test]
    fn a_contact_is_a_point_only_where_its_bound_is_decided_within_the_band() {
        let found = |bound: f64| Found {
            at: Vec3::new(1.0, 0.0, 0.0),
            on: (UvPoint::new(0.0, 0.0), UvPoint::new(1.0, 0.0)),
            bound,
        };
        let (mut decide, mut open) = (Decide::new(BAND), Vec::new());
        assert!(matches!(decided(found(0.5 * BAND), &mut decide, &mut open), Contact::Point { .. }));
        assert!(matches!(decided(found(2.0 * BAND), &mut decide, &mut open), Contact::BelowResolution { .. }));
        assert_eq!(decide.tally.decided, 2);
        assert!(matches!(decided(found(f64::INFINITY), &mut decide, &mut open), Contact::BelowResolution { .. }));
        assert_eq!((decide.tally.decided, decide.tally.undecided), (2, 0), "an infinite bound is not decided");
        assert!(matches!(decided(found(BAND), &mut decide, &mut open), Contact::BelowResolution { .. }));
        assert_eq!(open.len(), 1, "on the band's edge, undecided, and given back");
    }

    #[test]
    fn a_place_is_held_by_a_bound_it_lies_within() {
        let mut decide = Decide::new(BAND);
        let (origin, at) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 4.0, 0.0));
        assert_eq!(holds(origin, 5.0 + 2.0 * BAND, at, &mut decide), Ok(true));
        assert_eq!(holds(origin, 5.0 - 2.0 * BAND, at, &mut decide), Ok(false));
        assert_eq!(holds(origin, f64::INFINITY, at, &mut decide), Ok(true));
        assert_eq!(decide.tally.decided, 2);
    }

    #[test]
    fn a_small_move_is_carried_to_parameters_to_first_order() {
        let plane = level(0.0);
        assert_eq!(nudged(&plane, UvPoint::new(1.0, 2.0), Vec3::new(0.5, -0.25, 3.0)), UvPoint::new(1.5, 1.75));
    }
}
