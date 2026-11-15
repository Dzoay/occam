//! Source: original
//! The fit against traces whose curves are known: a circle on a torus, a closed NURBS's seam, a sphere's poles, a
//! patch's edges and a double cone's lower nappe. Each curve is measured between the trace's stations, more densely
//! than the trace went, against both surfaces and against its images. Then a trace too coarse for its fit to reach the
//! band, a fit the band cannot place, and fits at one and many threads.

use std::f64::consts::TAU;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};

use occam_geom::{Cone, Content, ParamRange, Scratch, Surface, Torus, Wrap};
use occam_math::{Exec, Tally, sin_cos};
use occam_report::{Code, Fault, NamedValue};
use occam_units::{Angle, Point, UvPoint, World};

use crate::trace_tests::{
    BAND, ball, gap, level, mm, moved, nurbs, nurbs_cylinder, point, radius, start_near, tol, tracer, upright,
};
use crate::{End, Fitted, SectionCurve, Start, Station, SurfacePair, Trace, fit, fits};

/// Two surfaces, a trace of where they meet, its fit, and how many curves it should give.
struct Case {
    name: &'static str,
    a: Box<dyn Surface>,
    b: Box<dyn Surface>,
    trace: Trace,
    fitted: Fitted,
    curves: usize,
}

/// The case of `a` and `b` traced from `start` with a fine tracer.
fn case(name: &'static str, (a, b): (Box<dyn Surface>, Box<dyn Surface>), start: Start, curves: usize) -> Case {
    let trace = tracer(1e-4, 0.5, 20_000).trace(a.as_ref(), b.as_ref(), start);
    let fitted = fit(a.as_ref(), b.as_ref(), &trace, tol());
    Case { name, a, b, trace, fitted, curves }
}

/// The plane `z = 0.5` and the torus of radii 10 and 1 about Z, which meet in two circles.
fn torus_pair() -> (Box<dyn Surface>, Box<dyn Surface>) {
    (Box::new(level(0.5)), Box::new(Torus::placed(&moved([0.0; 3]), mm(10.0), mm(1.0)).unwrap()))
}

/// The start on the torus's inner circle at the angle `turn`.
fn on_the_inner_circle(pair: &(Box<dyn Surface>, Box<dyn Surface>), turn: f64) -> Start {
    let (sin, cos) = sin_cos(turn);
    let r = 10.0 - 0.75_f64.sqrt();
    start_near(pair.0.as_ref(), pair.1.as_ref(), point(r * cos, r * sin, 0.51))
}

fn cases() -> &'static [Case] {
    static CASES: OnceLock<Vec<Case>> = OnceLock::new();
    CASES.get_or_init(|| {
        let torus = torus_pair();
        let start = on_the_inner_circle(&torus, 0.4);
        let seam = (nurbs_cylinder((0.0, TAU)), level(0.3));
        let (sin, cos) = sin_cos(2.0);
        let at_seam = start_near(&seam.0, &seam.1, point(2.0 * cos, 2.0 * sin, 0.3));
        let poles = (upright(0.0), ball(2.0, [0.0; 3]));
        let at_poles = start_near(&poles.0, &poles.1, point(0.0, 2.0, 0.01));
        let patch = (nurbs_cylinder((0.5, 2.5)), nurbs(&ball(2.2, [0.0; 3]), (0.0, 0.2), (TAU, 0.8)));
        let (sin, cos) = sin_cos(1.5);
        let on_patch = start_near(&patch.0, &patch.1, point(2.0 * cos, 2.0 * sin, 0.84_f64.sqrt() + 0.01));
        let cone = Cone::placed(&moved([0.0; 3]), mm(1.0), Angle::from_radians(0.5)).unwrap();
        let (sin, cos) = sin_cos(0.5);
        let nappe = Start { on_a: UvPoint::new(0.0, 0.0), on_b: UvPoint::new(3.0 * sin / cos - 1.0, 0.0) };
        vec![
            case("a circle on a torus", torus, start, 1),
            case("a closed NURBS's seam", (Box::new(seam.0), Box::new(seam.1)), at_seam, 1),
            case("a sphere's poles", (Box::new(poles.0), Box::new(poles.1)), at_poles, 2),
            case("a patch's edges", (Box::new(patch.0), Box::new(patch.1)), on_patch, 1),
            case("a cone's lower nappe", (Box::new(cone), Box::new(level(-3.0))), nappe, 1),
        ]
    })
}

/// `count` parameters evenly over `curve`'s range, its ends included.
fn dense(curve: &SectionCurve, count: usize) -> Vec<f64> {
    let range = curve.curve.range();
    let steps = f64::from(u32::try_from(count).unwrap());
    (0..=count).map(|k| range.lo + (range.hi - range.lo) * f64::from(u32::try_from(k).unwrap()) / steps).collect()
}

/// More samples of each curve than the trace has stations: four to each of its steps.
fn denser(case: &Case) -> usize {
    case.trace.stations.len().saturating_mul(4)
}

/// The case's curves, which are as many as it should give.
fn curves(case: &Case) -> &[SectionCurve] {
    assert_eq!(case.fitted.curves.len(), case.curves, "{}", case.name);
    &case.fitted.curves
}

/// The cases' fits reached the band, made decisions and left none undecided, and gave the curves expected.
#[test]
fn every_case_is_fitted_within_the_band_with_its_decisions_counted() {
    for case in cases() {
        let Fitted { curves, failures, tally } = &case.fitted;
        assert!(failures.is_empty(), "{}: {failures:?}", case.name);
        assert!(tally.decided > 0 && tally.undecided == 0, "{}: {tally:?}", case.name);
        assert_eq!(curves.len(), case.curves, "{}: {:?}", case.name, case.trace.crossings);
        for curve in curves {
            let fit = curve.curve.fit().unwrap().millimetres();
            assert!(fit > 0.0 && fit <= BAND, "{}: {fit:e}", case.name);
            assert_eq!(curve.curve.family().0, "fitted");
            for image in [&curve.on_a, &curve.on_b] {
                let fit = image.fit().unwrap();
                assert!(fit < 1e-6, "{}: an image fits to {fit:e} in its surface's parameters", case.name);
            }
        }
    }
}

#[test]
fn every_fitted_curve_lies_within_its_fit_of_both_surfaces_between_the_stations() {
    for case in cases() {
        for curve in curves(case) {
            let fit = curve.curve.fit().unwrap().millimetres();
            lies_within(case.name, curve, (case.a.as_ref(), case.b.as_ref()), (denser(case), fit));
        }
    }
}

/// How far apart `uv` and `foot` are in `surface`'s parameters, a parameter that comes round taken the short way
/// round, and one whose ends meet in a seam, as a closed B-spline's do, taken on the side of it `uv` is on.
fn short_way(surface: &dyn Surface, uv: UvPoint, foot: UvPoint) -> f64 {
    let domain = surface.domain();
    let along = |d: f64, range: ParamRange| {
        let period = range.hi - range.lo;
        if range.wrap == Wrap::Periodic {
            let r = d.rem_euclid(period);
            return if r > period / 2.0 { r - period } else { r };
        }
        if d.abs() > period / 2.0 { d - period * d.signum() } else { d }
    };
    let (du, dv) = (along(uv.u() - foot.u(), domain.u), along(uv.v() - foot.v(), domain.v));
    let across = UvPoint::new(uv.u() - du, uv.v() - dv);
    let same = gap(surface.point(across), surface.point(foot)) <= BAND;
    let (du, dv) = if same { (du, dv) } else { (uv.u() - foot.u(), uv.v() - foot.v()) };
    (du * du + dv * dv).sqrt()
}

/// Whether `p` is within a step of a point `surface` declares a line of its parameters collapses to.
fn near_a_pole(surface: &dyn Surface, p: Point<World>) -> bool {
    surface.degeneracies().iter().any(|line| gap(line.point, p) < 0.05)
}

#[test]
fn its_images_map_onto_it_within_the_band_and_lie_within_their_fits() {
    let mut scratch = Scratch::default();
    for case in cases() {
        for curve in curves(case) {
            for (surface, image) in [(&case.a, &curve.on_a), (&case.b, &curve.on_b)] {
                let fit = image.fit().unwrap();
                for t in dense(curve, denser(case)) {
                    let (p, uv) = (curve.curve.point(t), image.uv(t));
                    let off = gap(surface.point(uv), p);
                    assert!(off <= BAND, "{} at {t}: the image maps {off:e} from the curve", case.name);
                    if near_a_pole(surface.as_ref(), p) {
                        continue;
                    }
                    let foot = surface.invert(p, Some(uv), tol(), &mut scratch).unwrap();
                    let apart = short_way(surface.as_ref(), uv, foot);
                    assert!(apart <= fit, "{} at {t}: the image is {apart:e} from the foot, fit {fit:e}", case.name);
                }
            }
        }
    }
}

/// The parameters of an image's two ends.
fn ends(image: &dyn occam_geom::ParamCurve) -> [UvPoint; 2] {
    let range = image.range();
    [image.uv(range.lo), image.uv(range.hi)]
}

/// The `u` of an image's two ends, the lower first.
fn ends_u(image: &dyn occam_geom::ParamCurve) -> [f64; 2] {
    let mut u = ends(image).map(UvPoint::u);
    u.sort_by(f64::total_cmp);
    u
}

#[test]
fn a_trace_is_cut_where_a_parameter_jumps_and_closed_where_it_closes() {
    let [torus, seam, poles, patch, _] = cases() else { panic!() };
    let loop_ = &torus.fitted.curves[0].curve;
    let range = loop_.range();
    assert_eq!(loop_.point(range.lo), loop_.point(range.hi), "a closed trace with no crossing is a loop");
    let domain = seam.a.domain().u;
    assert_eq!(
        ends_u(seam.fitted.curves[0].on_a.as_ref()),
        [domain.lo, domain.hi],
        "cut at the seam, an end each side"
    );
    for curve in &poles.fitted.curves {
        for uv in ends(curve.on_b.as_ref()) {
            assert!((uv.v().abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-12, "from pole to pole: {uv:?}");
        }
    }
    assert_eq!(ends_u(patch.fitted.curves[0].on_a.as_ref()), [0.5, 2.5], "from one edge of the patch to the other");
    assert!(torus.fitted.curves[0].curve.range().hi > 0.0);
    let on_circle = |s: &Station| (radius(s) - (10.0 - 0.75_f64.sqrt())).abs() < 1e-6;
    assert!(torus.trace.stations.iter().all(on_circle) && torus.trace.ends == [End::Closed; 2]);
}

/// The trace of the torus's inner circle at steps of up to 1, far too coarse for a curve through its stations alone to
/// lie within the band.
fn coarse() -> (Box<dyn Surface>, Box<dyn Surface>, Trace) {
    let pair = torus_pair();
    let trace = tracer(1e-2, 1.0, 2000).trace(pair.0.as_ref(), pair.1.as_ref(), on_the_inner_circle(&pair, 0.4));
    (pair.0, pair.1, trace)
}

/// Every sample of `curve`, `count` of them, is within `within` of both surfaces.
fn lies_within(name: &str, curve: &SectionCurve, (a, b): (&dyn Surface, &dyn Surface), (count, within): (usize, f64)) {
    let mut scratch = Scratch::default();
    for t in dense(curve, count) {
        let p = curve.curve.point(t);
        for (surface, image) in [(a, &curve.on_a), (b, &curve.on_b)] {
            let off = surface.closest(p, Some(image.uv(t)), tol(), &mut scratch).unwrap().distance.millimetres();
            assert!(off <= within, "{name} at {t}: {off:e} against {within:e}");
        }
    }
}

#[test]
fn a_trace_too_coarse_for_its_stations_gains_new_ones_until_its_fit_reaches_the_band() {
    let (plane, torus, trace) = coarse();
    assert_eq!(trace.ends, [End::Closed; 2]);
    let Fitted { curves, failures, tally } = fit(plane.as_ref(), torus.as_ref(), &trace, tol());
    assert!(failures.is_empty(), "{failures:?}");
    let [curve] = curves.as_slice() else { panic!("{}", curves.len()) };
    let fit = curve.curve.fit().unwrap().millimetres();
    assert!(fit <= BAND && tally.decided > 0 && tally.undecided == 0, "{fit:e} {tally:?}");
    let range = curve.curve.range();
    let poles = curve.curve.to_bspline(range.lo, range.hi).unwrap().poles.len();
    assert!(poles > trace.stations.len().saturating_mul(2), "{poles} poles, {} stations", trace.stations.len());
    lies_within("the coarse trace", curve, (plane.as_ref(), torus.as_ref()), (poles.saturating_mul(4), fit));
}

#[test]
fn a_fit_that_cannot_reach_the_band_is_reported_with_what_it_reached() {
    // The stations lie on one plane and within the band of another parallel to it, which no corrector can settle
    // between, so the curve through them stays 0.9 of the band from the second.
    let (flat, above, trace) = on_two_planes(0.9 * BAND);
    let Fitted { curves, failures, tally } = fit(flat.as_ref(), above.as_ref(), &trace, tol());
    let [curve] = curves.as_slice() else { panic!("the curve is kept: {}", curves.len()) };
    let [report] = failures.as_slice() else { panic!("{failures:?}") };
    assert_eq!((report.code, report.fault), (Code::FitOutsideBand, Some(Fault::NumericallySensitive)));
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    let reached = curve.curve.fit().unwrap().millimetres();
    assert!((reached - 1.8 * BAND).abs() < 1e-20, "twice its gap: {reached:e}");
    assert_eq!((value("reached"), value("band")), (Some(reached), Some(BAND)));
    assert_eq!((value("points"), value("tolerance-in-force")), (Some(10.0), Some(BAND)));
    let Some(occam_report::Value::Point([east, north, up])) = report.value("at").map(|v| v.value) else { panic!() };
    let at = point(east, north, up);
    assert!(curve.curve.closest(at, None, tol(), &mut Scratch::default()).unwrap().distance.millimetres() < 1e-12);
    assert!(report.remedies.is_empty(), "no remedy an agent could follow");
    assert!(tally.decided > 0 && tally.undecided == 0, "{tally:?}");
    lies_within("the parallel planes", curve, (flat.as_ref(), above.as_ref()), (40, reached));
}

/// A hand-made trace of `stations`, which reached an edge at both ends.
fn traced(stations: Vec<Station>) -> Trace {
    Trace { stations, crossings: vec![], ends: [End::Edge; 2], steps: 0, tally: Tally::default(), undecided: vec![] }
}

/// `count` stations along the X axis a unit apart, on the plane `z = 0`, with their parameters on the plane
/// `z = height` those of the point `ahead` of each further along.
fn on_two_planes_by(height: f64, count: u8, ahead: impl Fn(u8) -> f64) -> (Box<dyn Surface>, Box<dyn Surface>, Trace) {
    let station = |k: u8| {
        let x = f64::from(k);
        Station { at: point(x, 0.0, 0.0), on_a: UvPoint::new(x, 0.0), on_b: UvPoint::new(x + ahead(k), 0.0) }
    };
    (Box::new(level(0.0)), Box::new(level(height)), traced((0..count).map(station).collect()))
}

/// Ten stations along the X axis, on the plane `z = 0` and with their parameters on the plane `z = height`.
fn on_two_planes(height: f64) -> (Box<dyn Surface>, Box<dyn Surface>, Trace) {
    on_two_planes_by(height, 10, |_| 0.0)
}

#[test]
fn a_gap_the_band_cannot_place_is_undecided() {
    let (a, b, trace) = on_two_planes(BAND / 2.0);
    let Fitted { curves, failures, tally } = fit(a.as_ref(), b.as_ref(), &trace, tol());
    let [report] = failures.as_slice() else { panic!("{failures:?}") };
    assert_eq!((report.code, report.fault), (Code::Undecided, Some(Fault::NumericallySensitive)));
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    let (quantity, radius) = (value("quantity").unwrap(), value("radius").unwrap());
    let at_the_band = (quantity.abs() - BAND).abs() <= radius && radius < 1e-20;
    assert!(at_the_band, "twice the distance, at the band: {quantity:e} {radius:e}");
    assert_eq!((value("band"), report.remedies.len()), (Some(BAND), 0));
    assert!(tally.undecided > 0, "{tally:?}");
    assert_eq!(curves.len(), 1, "the curve is kept beside it");
    for (height, code) in [(0.49 * BAND, None), (0.51 * BAND, Some(Code::FitOutsideBand))] {
        let (a, b, trace) = on_two_planes(height);
        let found = fit(a.as_ref(), b.as_ref(), &trace, tol());
        assert_eq!(found.failures.first().map(|r| r.code), code, "{height:e}");
        assert_eq!((found.curves.len(), found.tally.undecided), (1, 0));
        let reached = found.curves[0].curve.fit().unwrap().millimetres();
        assert!((reached - 2.0 * height).abs() < 1e-20, "{reached:e}");
    }
}

/// What a fit gives, as text: each curve's content and its images', with their fits, then the failures and the tally.
fn digest(fitted: &[Fitted]) -> Vec<String> {
    let words = |content: &mut Content| format!("{:?}", content.words());
    let mut out = Vec::new();
    for found in fitted {
        for curve in &found.curves {
            let (mut c, mut a, mut b) = (Content::default(), Content::default(), Content::default());
            curve.curve.content(&mut c);
            curve.on_a.content(&mut a);
            curve.on_b.content(&mut b);
            out.extend([words(&mut c), words(&mut a), words(&mut b)]);
        }
        out.push(format!("{:?} {:?}", found.failures, found.tally));
    }
    out
}

#[test]
fn fits_are_the_same_bits_at_one_and_many_threads() {
    let pair = torus_pair();
    let starts = [0.4, 1.9, 3.3, 5.0].map(|turn| on_the_inner_circle(&pair, turn));
    let steps = tracer(1e-4, 0.5, 20_000);
    let mut traces: Vec<Trace> = starts.iter().map(|s| steps.trace(pair.0.as_ref(), pair.1.as_ref(), *s)).collect();
    traces.push(coarse().2);
    let (a, b) = pair;
    let shared: SurfacePair = (Arc::from(a), Arc::from(b));
    let traces: Arc<[Trace]> = traces.into();
    let one = fits(&Exec::serial(), &shared, Arc::clone(&traces), tol());
    let many = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let other = fits(&many, &shared, Arc::clone(&traces), tol());
    assert_eq!(one.len(), traces.len());
    assert!(one.iter().all(|found| found.curves.len() == 1 && found.failures.is_empty()), "a curve from each");
    assert_eq!(digest(&other), digest(&one), "{} threads", many.thread_count());
    for (found, trace) in one.iter().zip(traces.iter()) {
        assert_eq!(
            digest(std::slice::from_ref(found)),
            digest(&[fit(shared.0.as_ref(), shared.1.as_ref(), trace, tol())])
        );
    }
}

#[test]
fn an_images_fit_is_twice_its_furthest_from_the_curves_feet_its_ends_included() {
    // Four stations, so that the curves are lines through the first and last. The image on the plane above runs
    // beside the curve by a tenth of the band, and by four tenths at one end, where it is furthest.
    for end in [0, 3] {
        let ahead = |k: u8| if k == end { 0.4 * BAND } else { 0.1 * BAND };
        let (flat, above, trace) = on_two_planes_by(0.1 * BAND, 4, ahead);
        let Fitted { curves, failures, .. } = fit(flat.as_ref(), above.as_ref(), &trace, tol());
        assert!(failures.is_empty(), "{failures:?}");
        let [curve] = curves.as_slice() else { panic!("{}", curves.len()) };
        let (on_flat, on_above) = (curve.on_a.fit().unwrap(), curve.on_b.fit().unwrap());
        assert!((on_above - 0.8 * BAND).abs() < 1e-14, "end {end}: {on_above:e}");
        assert!(on_flat < 1e-12, "the image on the plane it lies on fits to rounding: {on_flat:e}");
    }
}

/// Stations `y` apart up the Y axis, where the planes `z = 0` and `x = 0` meet, with their parameters on the first
/// wrong by `wrong` of each along the axis.
fn up_the_axis(ys: &[f64], wrong: impl Fn(usize) -> f64) -> Trace {
    let station = |(k, &y): (usize, &f64)| Station {
        at: point(0.0, y, 0.0),
        on_a: UvPoint::new(0.0, y + wrong(k)),
        on_b: UvPoint::new(0.0, y),
    };
    traced(ys.iter().enumerate().map(station).collect())
}

#[test]
fn refining_stops_when_its_rounds_are_done_and_reports_an_image_beyond_the_band() {
    // The first station's parameters on the floor are 1.5 of the band from its point, and a curve's first station is
    // always one it runs through. No station settled beside it brings the image there nearer, so each round settles
    // one more until the rounds are done.
    let (flat, wall) = (level(0.0), upright(0.0));
    let ys: Vec<f64> = (0..11_u8).map(|k| 0.1 * f64::from(k)).collect();
    let trace = up_the_axis(&ys, |k| if k == 0 { 1.5 * BAND } else { 0.0 });
    let Fitted { curves, failures, .. } = fit(&flat, &wall, &trace, tol());
    let [curve] = curves.as_slice() else { panic!("{}", curves.len()) };
    let [report] = failures.as_slice() else { panic!("{failures:?}") };
    let reached = report.value("reached").and_then(NamedValue::number).unwrap();
    assert_eq!(report.code, Code::FitOutsideBand);
    assert!((reached - 1.5 * BAND).abs() < 1e-20, "how far the image is, not the curve's fit: {reached:e}");
    assert!(curve.curve.fit().unwrap().millimetres() < 1e-12, "the curve itself lies on both");
    let range = curve.curve.range();
    let poles = curve.curve.to_bspline(range.lo, range.hi).unwrap().poles.len();
    assert!((12..=11 + 16).contains(&poles), "new stations, at most one a round: {poles}");
}

#[test]
fn a_piece_of_two_stations_is_a_line_and_one_of_fewer_is_nothing() {
    let (flat, wall) = (level(0.0), upright(0.0));
    let Fitted { curves, failures, .. } = fit(&flat, &wall, &up_the_axis(&[0.0, 2.0], |_| 0.0), tol());
    assert!(failures.is_empty(), "{failures:?}");
    let [line] = curves.as_slice() else { panic!("{}", curves.len()) };
    let range = line.curve.range();
    assert_eq!(line.curve.to_bspline(range.lo, range.hi).unwrap().degree, 1);
    assert!(gap(line.curve.point(f64::midpoint(range.lo, range.hi)), point(0.0, 1.0, 0.0)) < 1e-15);
    for few in [&[1.0][..], &[]] {
        let found = fit(&flat, &wall, &up_the_axis(few, |_| 0.0), tol());
        assert!(found.curves.is_empty() && found.failures.is_empty(), "{} stations", few.len());
    }
}
