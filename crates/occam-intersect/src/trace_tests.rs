//! Source: original
//! The trace against P5's failure modes, each built so that its answer is known: a corrector that jumps to a
//! neighbouring loop, a closed NURBS's seam, a sphere's pole, a double cone's lower nappe and a tangency; then a
//! patch's edges, and traces in rounds at one and many threads.

pub(crate) use occam_fixtures::{mm, point};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::num::NonZeroUsize;
use std::sync::Arc;

use occam_geom::{Cone, Cylinder, Plane, Scratch, Sphere, SplineSurface, Surface, Torus, UvBox};
use occam_math::{Exec, sin_cos};
use occam_report::{Code, Fault, NamedValue, Reason};
use occam_units::{Angle, Direction, Local, Placement, Point, Tolerance, UvPoint, Vec3, Vector, World};

use crate::correct::{Here, Pair};
use crate::decide::{Decide, touch};
use crate::{Budget, End, Jump, ROUND, Section, Start, Station, SurfacePair, Trace, Tracer, Which, Work};

pub(crate) const BAND: f64 = 1e-7;

/// How near a touch the trace must come: far nearer than the band alone puts it, which is anywhere within the
/// near-tangent shift `sqrt(2 r band)`, 6.3e-4 for a radius of 2, of where the surfaces touch.
pub(crate) const CONVERGED: f64 = 1e-6;

pub(crate) fn tol() -> Tolerance {
    occam_fixtures::tol_of(BAND)
}

pub(crate) fn tracer(sag: f64, longest: f64, steps: u32) -> Tracer {
    Tracer { tol: tol(), sag: mm(sag), longest: mm(longest), steps }
}

/// Turned by `angle` about `axis`, then moved to `to`.
pub(crate) fn placed(axis: [f64; 3], angle: f64, to: [f64; 3]) -> Placement<Local, World> {
    let spin =
        Placement::rotation(Direction::normalised(Vec3::new(axis[0], axis[1], axis[2])), Angle::from_radians(angle));
    let shift = Placement::translation(Vector::from_vec3(Vec3::new(to[0], to[1], to[2])));
    Placement::identity().then(&spin).then(&shift)
}

pub(crate) fn moved(to: [f64; 3]) -> Placement<Local, World> {
    placed([0.0, 0.0, 1.0], 0.0, to)
}

/// The plane `z = height`, its parameters along X and Y.
pub(crate) fn level(height: f64) -> Plane {
    Plane::placed(&moved([0.0, 0.0, height]))
}

/// The plane `x = across`, facing along X.
pub(crate) fn upright(across: f64) -> Plane {
    Plane::placed(&placed([0.0, 1.0, 0.0], FRAC_PI_2, [across, 0.0, 0.0]))
}

pub(crate) fn ball(radius: f64, centre: [f64; 3]) -> Sphere {
    Sphere::placed(&moved(centre), mm(radius)).unwrap()
}

/// `surface` over `patch` as the B-spline it is exactly there, which presents no analytic form.
pub(crate) fn nurbs(surface: &dyn Surface, lo: (f64, f64), hi: (f64, f64)) -> SplineSurface {
    let patch = UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1));
    SplineSurface::new(&surface.to_bspline(patch).unwrap()).unwrap()
}

/// The exact rational cylinder of radius 2 about Z, over the angles `turn` and the heights -1 to 1.
pub(crate) fn nurbs_cylinder(turn: (f64, f64)) -> SplineSurface {
    nurbs(&Cylinder::placed(&moved([0.0; 3]), mm(2.0)).unwrap(), (turn.0, -1.0), (turn.1, 1.0))
}

/// The start whose parameters on each surface are those of the foot of `near`.
pub(crate) fn start_near(a: &dyn Surface, b: &dyn Surface, near: Point<World>) -> Start {
    let mut scratch = Scratch::default();
    let on_a = a.invert(near, None, tol(), &mut scratch).unwrap();
    Start { on_a, on_b: b.invert(near, None, tol(), &mut scratch).unwrap() }
}

pub(crate) fn gap(p: Point<World>, q: Point<World>) -> f64 {
    (p.to_vec3() - q.to_vec3()).norm()
}

/// Every station is within the band of both surfaces at its parameters on each, inside both domains, and the band
/// decided every decision the trace made.
pub(crate) fn on_both(trace: &Trace, a: &dyn Surface, b: &dyn Surface) {
    assert_ne!(trace.stations.len(), 0, "{trace:?}");
    assert!(trace.tally.decided > 0 && trace.tally.undecided == 0, "{:?}", trace.tally);
    assert_eq!(trace.undecided, []);
    for (i, s) in trace.stations.iter().enumerate() {
        let (off_a, off_b) = (gap(a.point(s.on_a), s.at), gap(b.point(s.on_b), s.at));
        assert!(off_a <= BAND && off_b <= BAND, "station {i} is {off_a:e} and {off_b:e} off");
        assert!(a.domain().contains(s.on_a) && b.domain().contains(s.on_b), "station {i}: {s:?}");
    }
}

/// The length of a trace's polyline, the chord back to its first station included where it is closed.
fn length(trace: &Trace) -> f64 {
    let along: f64 = trace.stations.windows(2).map(|w| gap(w[0].at, w[1].at)).sum();
    let closing = match (trace.ends, trace.stations.first(), trace.stations.last()) {
        ([End::Closed, _], Some(first), Some(last)) => gap(first.at, last.at),
        _ => 0.0,
    };
    along + closing
}

/// The distance of a station from the Z axis.
pub(crate) fn radius(s: &Station) -> f64 {
    let v = s.at.to_vec3();
    (v.x * v.x + v.y * v.y).sqrt()
}

#[test]
fn a_corrector_that_jumps_to_a_neighbouring_loop_is_caught_as_a_stalled_trace() {
    // A torus of major radius 10 and minor 1 about Z, cut near the top of its tube in two circles 0.02 apart.
    let torus = Torus::placed(&moved([0.0; 3]), mm(10.0), mm(1.0)).unwrap();
    let height = (1.0 - 1e-4_f64).sqrt();
    let plane = level(height);
    let start = start_near(&plane, &torus, point(9.99, 0.0, height));
    let coarse = tracer(5e-3, 1.0, 600);
    let trace = coarse.trace(&plane, &torus, start);
    assert!(trace.stations.iter().any(|s| (radius(s) - 10.01).abs() < 1e-6), "it jumped to the outer loop");
    assert!(trace.ends.contains(&End::Stalled) && !trace.ends.contains(&End::Closed), "{:?}", trace.ends);
    assert!(trace.stations.len() > 100, "its stations are kept: {}", trace.stations.len());
    on_both(&trace, &plane, &torus);
    let reports = coarse.failures(&trace, &Section::default());
    let [report] = reports.as_slice() else { panic!("a stalled trace is reported: {reports:?}") };
    let found = (report.code, report.fault, report.reason);
    assert_eq!(found, (Code::Incomplete, Some(Fault::NumericallySensitive), Some(Reason::BudgetSpent)));
    let count = |name: &str| report.value(name).and_then(NamedValue::number);
    assert_eq!((count("budget"), count("curves"), count("contacts")), (Some(600.0), Some(0.0), Some(0.0)));
    let kept = f64::from(u32::try_from(trace.stations.len()).unwrap());
    assert_eq!(count("points"), Some(kept));
    let first = trace.stations[0].at.to_vec3();
    let shown = report.value("first").map(|value| value.value);
    assert_eq!(shown, Some(occam_report::Value::Point([first.x, first.y, first.z])));
    assert!(report.value("last").is_some() && report.remedies.is_empty());
    let fine = tracer(1e-5, 0.5, 20_000);
    let own = fine.trace(&plane, &torus, start);
    assert_eq!(own.ends, [End::Closed; 2], "finer steps keep to the start's own loop");
    assert!(own.stations.iter().all(|s| (radius(s) - 9.99).abs() < 1e-6));
    assert_eq!(fine.failures(&own, &Section::default()), []);
}

#[test]
fn a_trace_across_a_closed_nurbs_seam_continues() {
    let (cylinder, plane) = (nurbs_cylinder((0.0, TAU)), level(0.3));
    let (sin, cos) = sin_cos(2.0);
    let start = start_near(&cylinder, &plane, point(2.0 * cos, 2.0 * sin, 0.3));
    let trace = tracer(1e-4, 0.5, 5000).trace(&cylinder, &plane, start);
    assert_eq!(trace.ends, [End::Closed; 2]);
    let [crossing] = trace.crossings.as_slice() else { panic!("{:?}", trace.crossings) };
    assert_eq!((crossing.on, crossing.jump), (Which::A, Jump::Seam));
    let (near, far) = (trace.stations[crossing.at], trace.stations[crossing.at + 1]);
    assert_eq!(near.at, far.at);
    let (lo, hi) = (cylinder.domain().u.lo, cylinder.domain().u.hi);
    let mut ends = [near.on_a.u(), far.on_a.u()];
    ends.sort_by(f64::total_cmp);
    assert_eq!(ends, [lo, hi], "the parameter goes from one end of the seam to the other");
    on_both(&trace, &cylinder, &plane);
    assert!(trace.stations.iter().all(|s| (radius(s) - 2.0).abs() < 1e-6));
    assert!((length(&trace) - 2.0 * TAU).abs() < 1e-3, "all the way round: {}", length(&trace));
    let swapped = Start { on_a: start.on_b, on_b: start.on_a };
    let other = tracer(1e-4, 0.5, 5000).trace(&plane, &cylinder, swapped);
    let exchanged: Vec<_> = trace.stations.iter().map(|s| Station { on_a: s.on_b, on_b: s.on_a, ..*s }).collect();
    assert_eq!(other.stations, exchanged, "the other way round, the same stations with their images exchanged");
    assert_eq!((other.ends, other.crossings[0].on), (trace.ends, Which::B));
}

#[test]
fn a_touch_at_a_spheres_pole_is_classified_as_a_touch() {
    let (plane, sphere) = (level(2.0), ball(2.0, [0.0; 3]));
    let start = Start { on_a: UvPoint::new(0.05, -0.03), on_b: UvPoint::new(0.3, 1.4) };
    let trace = tracer(1e-4, 0.5, 2000).trace(&plane, &sphere, start);
    assert_eq!(trace.ends, [End::Touch; 2]);
    let [touch] = trace.stations.as_slice() else { panic!("{:?}", trace.stations) };
    assert!(gap(touch.at, point(0.0, 0.0, 2.0)) <= CONVERGED, "{touch:?}");
    assert!((touch.on_b.v() - FRAC_PI_2).abs() <= CONVERGED, "at the pole: {touch:?}");
    on_both(&trace, &plane, &sphere);
}

#[test]
fn a_curve_through_a_spheres_poles_carries_on_through_them() {
    let (plane, sphere) = (upright(0.0), ball(2.0, [0.0; 3]));
    let start = start_near(&plane, &sphere, point(0.0, 2.0, 0.01));
    let trace = tracer(1e-4, 0.5, 5000).trace(&plane, &sphere, start);
    assert_eq!(trace.ends, [End::Closed; 2], "a pole is not where the curve ends");
    let kinds: Vec<_> = trace.crossings.iter().map(|c| (c.on, c.jump)).collect();
    assert_eq!(kinds, [(Which::B, Jump::Pole); 2]);
    for crossing in &trace.crossings {
        let (near, far) = (trace.stations[crossing.at], trace.stations[crossing.at + 1]);
        assert_eq!(near.at, far.at);
        assert!((near.at.to_vec3().z.abs() - 2.0).abs() <= 10.0 * BAND, "at a pole: {near:?}");
        assert_eq!(near.on_b.v().abs(), FRAC_PI_2);
        assert_eq!(far.on_b.v(), near.on_b.v());
        let turned = (far.on_b.u() - near.on_b.u()).rem_euclid(TAU);
        assert!((turned - PI).abs() < 1e-3, "onto the opposite meridian: {near:?} {far:?}");
    }
    on_both(&trace, &plane, &sphere);
    assert!((length(&trace) - 2.0 * TAU).abs() < 1e-3, "all the way round: {}", length(&trace));
}

#[test]
fn a_double_cones_lower_nappe_inverts_to_the_right_parameters() {
    // The cone through the unit circle opening up Z at 0.5 has its apex at z = -cot 0.5; z = -3 cuts its lower nappe.
    let cone = Cone::placed(&moved([0.0; 3]), mm(1.0), Angle::from_radians(0.5)).unwrap();
    let plane = level(-3.0);
    let apex = cone.degeneracies()[0];
    let (sin, cos) = sin_cos(0.5);
    let across = (3.0 * sin / cos) - 1.0;
    let (on_plane, upper) = (UvPoint::new(across, 0.0), UvPoint::new(0.0, 0.0));
    let steps = tracer(1e-4, 0.5, 5000);
    for cone_first in [false, true] {
        let trace = if cone_first {
            steps.trace(&cone, &plane, Start { on_a: upper, on_b: on_plane })
        } else {
            steps.trace(&plane, &cone, Start { on_a: on_plane, on_b: upper })
        };
        assert_eq!(trace.ends, [End::Closed; 2], "cone first: {cone_first}");
        for s in &trace.stations {
            let on_cone = if cone_first { s.on_a } else { s.on_b };
            assert!(on_cone.v() < apex.at, "on the lower nappe: {s:?}");
            assert!(gap(cone.point(on_cone), s.at) <= BAND, "its parameters give its point: {s:?}");
        }
        assert!(trace.stations.iter().all(|s| (radius(s) - across).abs() < 1e-6));
        assert_eq!(trace.crossings, [], "an angle that comes round has no seam");
    }
}

#[test]
fn a_tangency_converges_by_projection() {
    let steps = tracer(1e-4, 0.5, 2000);
    let contact = point(2.0, 0.0, 0.0);
    let (plane, sphere) = (upright(2.0), ball(2.0, [0.0; 3]));
    let near = point(2.0, 0.05, -0.04);
    let touch = steps.trace(&plane, &sphere, start_near(&plane, &sphere, near));
    assert_eq!((touch.ends, touch.stations.len()), ([End::Touch; 2], 1), "{touch:?}");
    assert!(gap(touch.stations[0].at, contact) <= CONVERGED, "{touch:?}");
    on_both(&touch, &plane, &sphere);
    // Two rational sphere patches, of radii 2 and 1, touching outside each other: Newton on both parameters loses rank.
    let big = nurbs(&sphere, (-0.5, -0.5), (0.5, 0.5));
    let small = nurbs(&ball(1.0, [3.0, 0.0, 0.0]), (PI - 0.5, -0.5), (PI + 0.5, 0.5));
    let start = start_near(&big, &small, point(2.0, 0.06, 0.03));
    let touch = steps.trace(&big, &small, start);
    assert_eq!((touch.ends, touch.stations.len()), ([End::Touch; 2], 1), "{touch:?}");
    assert!(gap(touch.stations[0].at, contact) <= CONVERGED, "{touch:?}");
    on_both(&touch, &big, &small);
}

#[test]
fn a_curve_across_a_patch_ends_at_its_edges() {
    // The patch of the cylinder of radius 2 from 0.5 to 2.5 about Z, and a rational sphere of radius 2.2 about the
    // origin, which it meets in an arc of the circle of radius 2 at height sqrt(0.84).
    let patch = nurbs_cylinder((0.5, 2.5));
    let sphere = nurbs(&ball(2.2, [0.0; 3]), (0.0, 0.2), (TAU, 0.8));
    let height = 0.84_f64.sqrt();
    let (sin, cos) = sin_cos(1.5);
    let start = start_near(&patch, &sphere, point(2.0 * cos, 2.0 * sin, height + 0.01));
    let trace = tracer(1e-4, 0.5, 5000).trace(&patch, &sphere, start);
    assert_eq!(trace.ends, [End::Edge; 2]);
    assert!(trace.crossings.is_empty(), "the ends of the patch's angle do not meet: {:?}", trace.crossings);
    let mut ends = [trace.stations[0].on_a.u(), trace.stations[trace.stations.len() - 1].on_a.u()];
    ends.sort_by(f64::total_cmp);
    assert_eq!(ends, [patch.domain().u.lo, patch.domain().u.hi], "each end on an end of the patch's angle");
    on_both(&trace, &patch, &sphere);
    assert!(trace.stations.iter().all(|s| (radius(s) - 2.0).abs() < 1e-6 && (s.at.to_vec3().z - height).abs() < 1e-6));
    assert!((length(&trace) - 4.0).abs() < 1e-3, "the arc of two radians: {}", length(&trace));
}

/// The plane `z = 0.5` and the torus of major radius 10 and minor 1 about Z, which meet in two circles, and 40 starts
/// beside them, alternately on each.
fn many_starts() -> (SurfacePair, Vec<Start>) {
    let pair: SurfacePair =
        (Arc::new(level(0.5)), Arc::new(Torus::placed(&moved([0.0; 3]), mm(10.0), mm(1.0)).unwrap()));
    let radii = [10.0 - 0.75_f64.sqrt(), 10.0 + 0.75_f64.sqrt()];
    let starts = (0..40_u8)
        .map(|i| {
            let (sin, cos) = sin_cos(f64::from(i) * 0.3);
            let r = radii[usize::from(i % 2)];
            start_near(pair.0.as_ref(), pair.1.as_ref(), point(r * cos, r * sin, 0.51))
        })
        .collect();
    (pair, starts)
}

#[test]
fn traces_run_in_rounds_and_are_the_same_bits_at_one_and_many_threads() {
    let (pair, starts) = many_starts();
    let plane = Arc::clone(&pair.0);
    let skip = move |start: &Start, found: &[Trace]| {
        let at = plane.point(start.on_a);
        found.iter().flat_map(|trace| &trace.stations).any(|s| gap(s.at, at) < 1.0)
    };
    let steps = tracer(1e-2, 1.0, 2000);
    let run = |exec: &Exec, budget: u32| {
        let (mut found, mut work) = (Vec::new(), Work::of(Budget { steps: budget }));
        let spent = steps.traces(exec, &pair, &starts, &mut work, &mut found, &skip).is_err();
        (found, spent)
    };
    let (one, spent) = run(&Exec::serial(), u32::MAX);
    assert!(!spent);
    assert_eq!(one.len(), ROUND, "the first round traces every start in it, and the second skips the rest");
    assert!(one.iter().all(|trace| trace.ends == [End::Closed; 2] && trace.crossings.is_empty()));
    on_both(&one[0], pair.0.as_ref(), pair.1.as_ref());
    on_both(&one[1], pair.0.as_ref(), pair.1.as_ref());
    let many = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let (other, _) = run(&many, u32::MAX);
    assert_eq!(format!("{other:?}"), format!("{one:?}"), "{} threads", many.thread_count());
    let total = one.iter().map(|trace| trace.steps).sum::<u32>();
    assert_eq!(run(&many, total), (one.clone(), false), "every trace's steps are spent, and no more");
    let (short, spent) = run(&many, total - 1);
    assert!(spent);
    assert_eq!(short, one[..ROUND - 1], "a trace whose steps cannot be spent is dropped");
}

#[test]
fn a_touch_or_a_crossing_the_band_cannot_tell_apart_is_undecided() {
    // Two planes through the X axis at 1e-3 radians. At `longest` near `2 band / sin^2`, 0.2, a touch of that radius
    // would have a gap of the band where the normals are that far apart: at one such length, rounding alone decides.
    let (flat, tilted) = (level(0.0), Plane::placed(&placed([1.0, 0.0, 0.0], 1e-3, [0.0; 3])));
    let origin = Here { at: Vec3::new(0.0, 0.0, 0.0), p: UvPoint::new(0.0, 0.0), q: UvPoint::new(0.0, 0.0) };
    let normals = Pair::new(&flat, &tilted, tol()).normals(&origin).unwrap();
    let undecided = |longest: f64| Decide::new(BAND).side(touch(longest, normals), origin.at).is_err();
    let sin = normals.0.cross(normals.1).norm();
    let around = 2.0 * BAND / (sin * sin);
    let up = (0..64).scan(around, |x, _| Some(std::mem::replace(x, x.next_up())));
    let down = (0..64).scan(around, |x, _| Some(std::mem::replace(x, x.next_down())));
    let edge = up.chain(down).find(|&longest| undecided(longest)).expect("a length rounding alone decides at");
    let start = Start { on_a: UvPoint::new(0.3, 0.01), on_b: UvPoint::new(0.3, 0.01) };
    let steps = |longest: f64| Tracer { tol: tol(), sag: mm(1e-4), longest: mm(longest), steps: 200 };
    let trace = steps(edge).trace(&flat, &tilted, start);
    assert_eq!((trace.ends, trace.stations.len(), trace.tally.undecided), ([End::Undecided; 2], 1, 1), "{trace:?}");
    let [uncertain] = trace.undecided.as_slice() else { panic!("{:?}", trace.undecided) };
    assert!((uncertain.length.millimetres() - BAND).abs() <= uncertain.radius.millimetres(), "{uncertain:?}");
    let reports = steps(edge).failures(&trace, &Section::default());
    let [report] = reports.as_slice() else { panic!("{reports:?}") };
    assert_eq!((report.code, report.fault), (Code::Undecided, Some(Fault::NumericallySensitive)));
    let length = |name: &str| report.value(name).and_then(NamedValue::number);
    assert_eq!(length("quantity"), Some(uncertain.length.millimetres()));
    assert_eq!(length("radius"), Some(uncertain.radius.millimetres()));
    assert_eq!((length("band"), length("tolerance-in-force")), (Some(BAND), Some(BAND)));
    let at = trace.stations[0].at.to_vec3();
    assert_eq!(report.value("at").map(|value| value.value), Some(occam_report::Value::Point([at.x, at.y, at.z])));
    assert!(report.remedies.is_empty(), "no remedy: none is in the catalogue");
    // Either side of it, the band decides: a touch below, a crossing above, which the trace follows along the axis.
    let below = steps(edge * 0.999).trace(&flat, &tilted, start);
    assert_eq!((below.ends, below.tally.undecided), ([End::Touch; 2], 0), "{below:?}");
    let above = steps(edge * 1.001).trace(&flat, &tilted, start);
    assert_eq!((above.ends, above.tally.undecided), ([End::Stalled; 2], 0), "{:?}", above.ends);
    assert!(above.stations.iter().all(|s| s.at.to_vec3().y.abs() <= BAND && s.at.to_vec3().z.abs() <= BAND));
}
