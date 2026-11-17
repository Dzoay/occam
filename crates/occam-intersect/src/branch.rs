//! Source: original
//! Branch points in the hybrid (O24): where two branches of the section cross, the surfaces are tangent, and a trace
//! that reaches one with no station near enough for its touch test passes through it, as P5's did. Along a trace the
//! sine of the angle between the two surfaces' normals is least near such a point, so at each station where it is least
//! the gap between the surfaces is read as [`resolution`](crate::resolution)'s model reads it: where the relative
//! curvature has eigenvalues of both signs, the place where the gap is least or most lies within half a step of the
//! station on either side, and the gap there is decided within the band, the branches cross there. The trace is cut at
//! that station, which ends both its pieces as a touch, and the station is a contact by the rule. A station of a seam
//! or a pole the trace crossed is not cut again, as the fit cuts the curve there already, but is a contact all the same.

use occam_geom::Surface;
use occam_math::{Interval, Side};
use occam_units::Vec3;

use crate::decide::Decide;
use crate::resolution::Near;
use crate::trace::{Crossing, End, Station, Trace, Uncertain};

/// The unit normal of `surface` at `uv` from its derivatives, where they span a plane.
fn normal(surface: &dyn Surface, uv: occam_units::UvPoint) -> Option<Vec3> {
    let jet = surface.first(uv);
    let cross = jet.du.rate().cross(jet.dv.rate());
    let size = cross.norm();
    (size > 0.0 && size.is_finite()).then(|| cross * (1.0 / size))
}

/// The sine of the angle between the two surfaces' normals at `station`, or `None` where either has none.
fn sine((a, b): (&dyn Surface, &dyn Surface), station: &Station) -> Option<f64> {
    Some(normal(a, station.on_a)?.cross(normal(b, station.on_b)?).norm())
}

/// Whether the two branches of the section cross near `here`, between the stations either side of it: the relative
/// curvature there has eigenvalues of both signs, the place where the gap is least or most is within half the way to
/// the further neighbour, as one of a seam's two stations has the other at no distance, and the gap there is decided
/// within the band.
fn crosses(
    surfaces: (&dyn Surface, &dyn Surface),
    (before, here, after): (&Station, &Station, &Station),
    decide: &mut Decide,
) -> Result<bool, Uncertain> {
    let Some(gap) = Near::between(surfaces.0, here.on_a, surfaces.1, here.on_b) else { return Ok(false) };
    let [first, second] = gap.eigenvalues();
    if first.is_sign_negative() == second.is_sign_negative() || gap.flat() {
        return Ok(false);
    }
    let offset = gap.lifted(gap.critical()).norm();
    let reach = |other: &Station| 0.5 * (other.at.to_vec3() - here.at.to_vec3()).norm();
    let (back, on) = (reach(before), reach(after));
    if offset > (if back > on { back } else { on }) {
        return Ok(false);
    }
    Ok(decide.side(Interval::exact(gap.extremal()), here.at.to_vec3())? == Side::Within)
}

/// The stations of `trace` where two branches cross, by index, each where the sine between the normals is least.
fn found(
    surfaces: (&dyn Surface, &dyn Surface),
    trace: &Trace,
    decide: &mut Decide,
    uncertain: &mut Vec<Uncertain>,
) -> Vec<usize> {
    let stations = trace.stations.as_slice();
    let count = stations.len();
    let closed = trace.ends.contains(&End::Closed);
    if count < 3 {
        return Vec::new();
    }
    let sines: Vec<f64> = stations.iter().map(|s| sine(surfaces, s).unwrap_or(f64::NAN)).collect();
    let mut out = Vec::new();
    for at in 0..count {
        let (Some(back), Some(on)) = (neighbour(at, count, closed, false), neighbour(at, count, closed, true)) else {
            continue;
        };
        let (s, b, n) = (sines.get(at), sines.get(back), sines.get(on));
        let (Some(&s), Some(&b), Some(&n)) = (s, b, n) else { continue };
        if !(s < b && s <= n) {
            continue;
        }
        let (Some(before), Some(here), Some(after)) = (stations.get(back), stations.get(at), stations.get(on)) else {
            continue;
        };
        match crosses(surfaces, (before, here, after), decide) {
            Ok(true) => out.push(at),
            Ok(false) => {}
            Err(open) => uncertain.push(open),
        }
    }
    out
}

/// The index of the station before `at`, or after it where `ahead`, round the trace where it is closed.
fn neighbour(at: usize, count: usize, closed: bool, ahead: bool) -> Option<usize> {
    match (ahead, closed) {
        (true, _) if at.saturating_add(1) < count => Some(at.saturating_add(1)),
        (true, true) => Some(0),
        (false, _) if at > 0 => Some(at.saturating_sub(1)),
        (false, true) => count.checked_sub(1),
        _ => None,
    }
}

/// Whether station `at` is one of a seam's or a pole's two.
fn crossed(trace: &Trace, at: usize) -> bool {
    trace.crossings.iter().any(|c| c.at == at || c.at.saturating_add(1) == at)
}

/// `trace`'s stations from `from` to `to`, both included, with the crossings among them, ending as `ends`.
fn part(trace: &Trace, (from, to): (usize, usize), ends: [End; 2]) -> Trace {
    let stations = trace.stations.get(from..=to).unwrap_or_default().to_vec();
    let crossings: Vec<Crossing> = trace
        .crossings
        .iter()
        .filter(|c| c.at >= from && c.at < to)
        .map(|c| Crossing { at: c.at.saturating_sub(from), ..*c })
        .collect();
    Trace { stations, crossings, ends, steps: 0, tally: occam_math::Tally::default(), undecided: Vec::new() }
}

/// `trace` cut at the stations `at`, in order, each piece ending in a touch there: an open trace into one piece more
/// than its cuts, a closed one, turned to start at its first cut, into as many as its cuts. The first piece keeps the
/// trace's steps, tally and undecided decisions, so that none is counted twice.
fn cut(trace: &Trace, at: &[usize]) -> Vec<Trace> {
    let Some(&first) = at.first() else { return vec![trace.clone()] };
    let closed = trace.ends.contains(&End::Closed);
    let turned = if closed { turn(trace, first) } else { trace.clone() };
    let shift = |k: usize| if closed { k.saturating_sub(first) } else { k };
    let mut bounds: Vec<usize> = vec![0];
    bounds.extend(at.iter().map(|k| shift(*k)));
    bounds.push(turned.stations.len().saturating_sub(1));
    bounds.dedup();
    let last = bounds.len().saturating_sub(2);
    let mut out = Vec::new();
    for (index, pair) in bounds.windows(2).enumerate() {
        let (Some(&from), Some(&to)) = (pair.first(), pair.last()) else { continue };
        let start = if index == 0 && !closed { turned.ends[0] } else { End::Touch };
        let end = if index == last && !closed { turned.ends[1] } else { End::Touch };
        out.push(part(&turned, (from, to), [start, end]));
    }
    if let Some(head) = out.first_mut() {
        head.steps = trace.steps;
        head.tally = trace.tally;
        head.undecided.clone_from(&trace.undecided);
    }
    out
}

/// A closed `trace` turned to start at station `from`, and to end there again, open: the closing chord becomes one of
/// its own, and every crossing keeps its stations.
fn turn(trace: &Trace, from: usize) -> Trace {
    let count = trace.stations.len();
    let wrap = |k: usize| k.checked_rem(count).unwrap_or(0);
    let stations: Vec<Station> =
        (0..=count).filter_map(|k| trace.stations.get(wrap(from.saturating_add(k)))).copied().collect();
    let crossings = trace
        .crossings
        .iter()
        .map(|c| Crossing { at: wrap(c.at.saturating_add(count).saturating_sub(from)), ..*c })
        .collect();
    Trace { stations, crossings, ends: [End::Touch; 2], ..trace.clone() }
}

/// What finding branch points gave: the traces cut at them, and the stations that are contacts.
pub(crate) struct Branched {
    pub traces: Vec<Trace>,
    pub contacts: Vec<Station>,
}

/// Each of `kept` cut at the branch points along it, between `surfaces`, and those points as contacts.
pub(crate) fn branched(
    surfaces: (&dyn Surface, &dyn Surface),
    kept: Vec<Trace>,
    decide: &mut Decide,
    uncertain: &mut Vec<Uncertain>,
) -> Branched {
    let mut out = Branched { traces: Vec::new(), contacts: Vec::new() };
    for trace in kept {
        let at = found(surfaces, &trace, decide, uncertain);
        out.contacts.extend(at.iter().filter_map(|k| trace.stations.get(*k)).copied());
        let cuts: Vec<usize> = at.into_iter().filter(|k| !crossed(&trace, *k)).collect();
        out.traces.extend(cut(&trace, &cuts));
    }
    out
}

#[cfg(test)]
mod tests {
    use occam_geom::Cylinder;
    use occam_math::{Tally, sin_cos};
    use occam_units::{Point, UvPoint};

    use super::*;
    use crate::trace::Jump;
    use crate::trace::Which;
    use crate::trace_tests::{BAND, ball, level, mm, moved, placed, tol};

    /// The station at `at` on `a` and `b`, its parameters each's inverse of it.
    fn station(a: &dyn Surface, b: &dyn Surface, at: Vec3) -> Station {
        let (point, mut scratch) = (Point::from_vec3(at), occam_geom::Scratch::default());
        let on_a = a.invert(point, None, tol(), &mut scratch).unwrap();
        Station { at: point, on_a, on_b: b.invert(point, None, tol(), &mut scratch).unwrap() }
    }

    /// Stations at `x` along the X axis.
    fn along(xs: &[f64]) -> Vec<Station> {
        let s = |x: &f64| Station {
            at: Point::from_vec3(Vec3::new(*x, 0.0, 0.0)),
            on_a: UvPoint::new(*x, 0.0),
            on_b: UvPoint::new(*x, 1.0),
        };
        xs.iter().map(s).collect()
    }

    fn trace(stations: Vec<Station>, crossings: Vec<Crossing>, ends: [End; 2]) -> Trace {
        Trace {
            stations,
            crossings,
            ends,
            steps: 7,
            tally: Tally { decided: 3, exact: 0, undecided: 0 },
            undecided: vec![],
        }
    }

    fn seam(at: usize) -> Crossing {
        Crossing { at, on: Which::B, jump: Jump::Seam }
    }

    fn xs(trace: &Trace) -> Vec<f64> {
        trace.stations.iter().map(|s| s.at.to_vec3().x).collect()
    }

    /// The unit cylinders about Z and about X, Steinmetz's pair, and stations along their ellipse in the plane `x = z`
    /// at angles about a quarter turn, where it crosses the other ellipse at `(0, 1, 0)`.
    fn steinmetz(turns: &[f64]) -> (Cylinder, Cylinder, Vec<Station>) {
        let upright = Cylinder::placed(&moved([0.0; 3]), mm(1.0)).unwrap();
        let across =
            Cylinder::placed(&placed([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2, [0.0; 3]), mm(1.0)).unwrap();
        let on = |t: &f64| {
            let (s, c) = sin_cos(*t);
            station(&upright, &across, Vec3::new(c, s, c))
        };
        let stations = turns.iter().map(on).collect();
        (upright, across, stations)
    }

    #[test]
    fn a_stations_neighbours_are_the_ones_beside_it_round_a_closed_trace() {
        assert_eq!((neighbour(0, 4, false, false), neighbour(3, 4, false, true)), (None, None));
        assert_eq!((neighbour(0, 4, true, false), neighbour(3, 4, true, true)), (Some(3), Some(0)));
        assert_eq!((neighbour(2, 4, false, false), neighbour(2, 4, false, true)), (Some(1), Some(3)));
        assert_eq!(neighbour(0, 0, true, false), None);
    }

    #[test]
    fn a_seams_two_stations_are_crossed() {
        let t = trace(along(&[0.0, 1.0, 1.0, 2.0]), vec![seam(1)], [End::Edge; 2]);
        assert_eq!((0..4).map(|k| crossed(&t, k)).collect::<Vec<_>>(), [false, true, true, false]);
    }

    #[test]
    fn an_open_trace_cut_at_a_station_is_two_pieces_ending_in_a_touch_there() {
        let t = trace(along(&[0.0, 1.0, 2.0, 2.0, 3.0, 4.0, 5.0]), vec![seam(2)], [End::Edge, End::Stalled]);
        let pieces = cut(&t, &[4]);
        let [first, second] = pieces.as_slice() else { panic!("{pieces:?}") };
        assert_eq!(
            (xs(first), first.ends, first.crossings.clone()),
            (vec![0.0, 1.0, 2.0, 2.0, 3.0], [End::Edge, End::Touch], vec![seam(2)])
        );
        assert_eq!(
            (xs(second), second.ends, second.crossings.len()),
            (vec![3.0, 4.0, 5.0], [End::Touch, End::Stalled], 0)
        );
        assert_eq!((first.steps, first.tally.decided, second.steps, second.tally.decided), (7, 3, 0, 0));
        assert_eq!(cut(&t, &[]), vec![t.clone()]);
        let later = cut(&t, &[5]);
        assert_eq!(later.iter().map(|p| p.crossings.len()).collect::<Vec<_>>(), [1, 0]);
    }

    #[test]
    fn a_closed_trace_cut_at_two_stations_is_two_pieces_round_it_each_ending_in_touches() {
        let t = trace(along(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]), vec![seam(4)], [End::Closed; 2]);
        let pieces = cut(&t, &[1, 4]);
        let [first, second] = pieces.as_slice() else { panic!("{pieces:?}") };
        assert_eq!((xs(first), first.ends), (vec![1.0, 2.0, 3.0, 4.0], [End::Touch; 2]));
        assert_eq!((xs(second), second.ends), (vec![4.0, 5.0, 0.0, 1.0], [End::Touch; 2]));
        assert_eq!((first.crossings.len(), second.crossings.clone()), (0, vec![seam(0)]));
        let once = cut(&t, &[3]);
        let [round] = once.as_slice() else { panic!("{once:?}") };
        assert_eq!((xs(round), round.crossings.clone()), (vec![3.0, 4.0, 5.0, 0.0, 1.0, 2.0, 3.0], vec![seam(1)]));
    }

    #[test]
    fn a_crossing_is_kept_only_by_the_piece_that_holds_both_its_stations() {
        let t = trace(along(&[0.0, 1.0, 2.0, 2.0, 3.0]), vec![seam(2)], [End::Edge; 2]);
        let at = |k: usize| cut(&t, &[k]).iter().map(|p| p.crossings.len()).collect::<Vec<_>>();
        assert_eq!((at(2), at(3), at(1)), (vec![0, 1], vec![1, 0], vec![0, 1]));
    }

    #[test]
    fn a_normal_is_of_unit_length_whatever_the_parameters_speed() {
        // On a sphere of radius 2 the derivatives' cross product is 4 cos v long; the normal is the radius over 2.
        let sphere = ball(2.0, [1.0, 0.0, 0.0]);
        let uv = UvPoint::new(0.4, 0.3);
        let got = normal(&sphere, uv).unwrap();
        let want = (sphere.point(uv).to_vec3() - Vec3::new(1.0, 0.0, 0.0)) * 0.5;
        assert!((got - want).norm() <= 1e-15 || (got + want).norm() <= 1e-15, "{got:?} against {want:?}");
    }

    #[test]
    fn a_surface_with_no_normal_there_has_no_sine() {
        let cone = occam_geom::Cone::placed(&moved([0.0; 3]), mm(0.0), occam_units::Angle::from_radians(0.5)).unwrap();
        let apex = Station {
            at: Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)),
            on_a: UvPoint::new(0.0, 0.0),
            on_b: UvPoint::new(0.3, 0.0),
        };
        assert_eq!((normal(&cone, apex.on_b), sine((&level(0.0), &cone), &apex)), (None, None));
    }

    #[test]
    fn a_closed_trace_turned_starts_and_ends_at_the_station_given() {
        let t = trace(along(&[0.0, 1.0, 2.0]), vec![seam(2)], [End::Closed; 2]);
        let turned = turn(&t, 1);
        assert_eq!(
            (xs(&turned), turned.crossings.clone(), turned.ends),
            (vec![1.0, 2.0, 0.0, 1.0], vec![seam(1)], [End::Touch; 2])
        );
    }

    #[test]
    fn the_sine_between_two_planes_normals_is_that_of_their_angle() {
        let tilted = occam_geom::Plane::placed(&placed([1.0, 0.0, 0.0], 0.6, [0.0; 3]));
        let at = station(&level(0.0), &tilted, Vec3::new(0.3, 0.0, 0.0));
        let got = sine((&level(0.0), &tilted), &at).unwrap();
        assert!((got - sin_cos(0.6).0).abs() <= 1e-12, "{got}");
    }

    #[test]
    fn two_cylinders_crossing_branches_are_found_where_the_sine_between_their_normals_is_least() {
        let turns = [1.2, 1.35, 1.5, std::f64::consts::FRAC_PI_2, 1.65, 1.8, 1.95];
        let (upright, across, stations) = steinmetz(&turns);
        let (mut decide, mut open) = (Decide::new(BAND), Vec::new());
        let open_trace = trace(stations.clone(), vec![], [End::Edge; 2]);
        assert_eq!(found((&upright, &across), &open_trace, &mut decide, &mut open), [3]);
        let off = trace(stations.iter().take(3).copied().collect(), vec![], [End::Edge; 2]);
        assert_eq!(found((&upright, &across), &off, &mut decide, &mut open), Vec::<usize>::new(), "no least inside");
        let short = trace(stations.iter().take(2).copied().collect(), vec![], [End::Edge; 2]);
        assert_eq!(found((&upright, &across), &short, &mut decide, &mut open), Vec::<usize>::new());
        let three = trace(stations.iter().skip(2).take(3).copied().collect(), vec![], [End::Edge; 2]);
        assert_eq!(found((&upright, &across), &three, &mut decide, &mut open), [1], "three stations are enough");
        assert!(open.is_empty() && decide.tally.decided >= 1, "{open:?}");
        let got = branched((&upright, &across), vec![open_trace], &mut decide, &mut open);
        assert_eq!((got.traces.len(), got.contacts.len()), (2, 1));
        assert!((got.contacts[0].at.to_vec3() - Vec3::new(0.0, 1.0, 0.0)).norm() <= 1e-9);
    }

    #[test]
    fn two_cylinders_crossing_at_a_slant_are_a_saddle_however_the_plane_is_turned() {
        // The unit cylinders about Z and about (sin 0.5, 0, cos 0.5) touch at (0, 1, 0), where the relative
        // curvature, over X and Z, is [[-s², -c s], [-c s, s²]]: a saddle whose axes are not the plane's.
        let (s, c) = sin_cos(0.5);
        let upright = Cylinder::placed(&moved([0.0; 3]), mm(1.0)).unwrap();
        let slant = Cylinder::placed(&placed([0.0, 1.0, 0.0], 0.5, [0.0; 3]), mm(1.0)).unwrap();
        let node = station(&upright, &slant, Vec3::new(0.0, 1.0, 0.0));
        let near = |x: f64| Station { at: Point::from_vec3(Vec3::new(x, 1.0, 0.0)), ..node };
        let model = Near::between(&upright, node.on_a, &slant, node.on_b).unwrap();
        let [[k11, k12], [_, k22]] = model.bend;
        assert!((k11 * k22 - k12 * k12 + s * s).abs() <= 1e-12 && (k12.abs() - c * s).abs() <= 1e-12, "{model:?}");
        let mut decide = Decide::new(BAND);
        assert_eq!(crosses((&upright, &slant), (&near(-0.1), &node, &near(0.1)), &mut decide), Ok(true));
    }

    #[test]
    fn a_station_beside_the_node_is_not_it_where_the_node_lies_beyond_half_its_steps() {
        // On an ellipse of the slanted pair, 1e-3 round from the node, with neighbours 1e-4 either side: the node is
        // further than half a step, so this station is not where the branches cross; at the node itself it is.
        let (s, c) = sin_cos(0.5);
        let upright = Cylinder::placed(&moved([0.0; 3]), mm(1.0)).unwrap();
        let slant = Cylinder::placed(&placed([0.0, 1.0, 0.0], 0.5, [0.0; 3]), mm(1.0)).unwrap();
        let normal = Vec3::new(0.0, 1.0, 0.0).cross(Vec3::new(s, 0.0, 1.0 + c));
        let on = |t: f64| {
            let (sine, cosine) = sin_cos(t);
            station(&upright, &slant, Vec3::new(cosine, sine, -normal.x * cosine / normal.z))
        };
        let node = std::f64::consts::FRAC_PI_2;
        let mut decide = Decide::new(BAND);
        let at =
            |t: f64, decide: &mut Decide| crosses((&upright, &slant), (&on(t - 1e-4), &on(t), &on(t + 1e-4)), decide);
        assert_eq!((at(node + 1e-3, &mut decide), at(node, &mut decide)), (Ok(false), Ok(true)));
    }

    #[test]
    fn a_touch_from_above_or_below_is_no_saddle() {
        // A sphere resting on a plane or hanging under it: the relative curvature has one sign either way.
        // From above the relative curvature is positive, from below negative.
        let plane = level(0.0);
        let mut decide = Decide::new(BAND);
        for (height, sign) in [(2.0, 1.0), (-2.0, -1.0)] {
            let sphere = ball(2.0, [0.0, 0.0, height]);
            let here = station(&plane, &sphere, Vec3::new(0.0, 0.0, 0.0));
            let near = |x: f64| Station { at: Point::from_vec3(Vec3::new(x, 0.0, 0.0)), ..here };
            let model = Near::between(&plane, here.on_a, &sphere, here.on_b).unwrap();
            assert!(model.eigenvalues().iter().all(|k| (k - 0.5 * sign).abs() <= 1e-12), "{model:?}");
            assert_eq!(crosses((&plane, &sphere), (&near(-0.1), &here, &near(0.1)), &mut decide), Ok(false));
        }
    }

    #[test]
    fn a_branch_point_at_a_seam_is_a_contact_but_not_cut_again() {
        let turns = [1.2, 1.35, 1.5, std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2, 1.65, 1.8];
        let (upright, across, stations) = steinmetz(&turns);
        let (mut decide, mut open) = (Decide::new(BAND), Vec::new());
        let t = trace(stations, vec![seam(3)], [End::Edge; 2]);
        let got = branched((&upright, &across), vec![t], &mut decide, &mut open);
        assert_eq!((got.traces.len(), got.contacts.len()), (1, 1));
    }

    #[test]
    fn where_the_branches_miss_by_more_than_the_band_or_the_contact_is_a_touch_no_branch_is_found() {
        // The cylinder about X lifted by 1e-4: the gap where the normals are parallel is 1e-4, above the band.
        let upright = Cylinder::placed(&moved([0.0; 3]), mm(1.0)).unwrap();
        let lifted =
            Cylinder::placed(&placed([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2, [0.0, 1e-4, 0.0]), mm(1.0)).unwrap();
        let near = |t: f64| {
            let (s, c) = sin_cos(t);
            let p = Vec3::new(c, s, c);
            let mut scratch = occam_geom::Scratch::default();
            let on_a = upright.invert(Point::from_vec3(p), None, tol(), &mut scratch).unwrap();
            let on_b = lifted.invert(Point::from_vec3(p), None, tol(), &mut scratch).unwrap();
            Station { at: Point::from_vec3(p), on_a, on_b }
        };
        let stations: Vec<Station> =
            [1.3, 1.45, std::f64::consts::FRAC_PI_2, 1.7, 1.85].into_iter().map(near).collect();
        let (mut decide, mut open) = (Decide::new(BAND), Vec::new());
        let t = trace(stations, vec![], [End::Edge; 2]);
        assert_eq!(found((&upright, &lifted), &t, &mut decide, &mut open), Vec::<usize>::new());
        // A sphere touching a plane: the relative curvature has one sign.
        let (plane, sphere) = (level(0.0), ball(2.0, [0.0, 0.0, 2.0]));
        let s = |x: f64| station(&plane, &sphere, Vec3::new(x, 0.0, 2.0 - (4.0 - x * x).sqrt()));
        let t = trace(vec![s(-0.1), s(0.0), s(0.1)], vec![], [End::Edge; 2]);
        assert_eq!(found((&plane, &sphere), &t, &mut decide, &mut open), Vec::<usize>::new());
    }
}
