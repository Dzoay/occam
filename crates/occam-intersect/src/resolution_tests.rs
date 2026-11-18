//! Source: original
//! What the tolerance cannot tell apart (O24), against P5's cases rebuilt with their answers known by construction and
//! placed by P5's rigid placement (`docs/evidence/2026-11-09-p5.md`). P5's near-tangent sweep: two spheres a gap `ε`
//! into each other, a plane `ε` inside a cylinder's side, and two bicubic graphs whose difference is
//! `(x − 0.37)² + (y + 0.41)² − ε` or `(x − 0.2)² − ε`, each at twelve gaps from 1e-1 to 1e-12, through the closed
//! forms, the pencil method and the hybrid as `booleans` asks them. Each case must be correct or below the resolution
//! with a bound that holds its true section, and none silent; `nt_poly_loop` at 1e-10, P5's one silent failure, is below
//! the resolution. Then the branch points: Viviani's node cuts its curve, in the pencil method and in the hybrid.

use std::f64::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use occam_geom::{BsplineSurface, Curve, Cylinder, Scratch, Sphere, SplineSurface, Surface};
use occam_math::{sin_cos, smaller};
use occam_units::{Angle, Bounds, Length, Point, Tolerance, Vec3};

use crate::closed::tests::{O, X, Z, circle, cylinder, from_curve, placed, plane, sphere, v};
use crate::trace_tests::{mm, moved, nurbs, placed as turned};
use crate::{Budget, Closed, Contact, Hybrid, Methods, Pencil, Section, SectionCurve, Tracer};

/// The band the sweep is decided at: P5's tolerance of 1e-10, halved, so that no gap of the sweep is on its edge.
const BAND: f64 = 5e-11;

/// P5's length scale.
const SCALE: f64 = 4.0;

fn tol() -> Tolerance {
    occam_fixtures::tol_of(BAND)
}

/// The blossom of `x^power`, of degree three, at `knots`: the average of the products of `power` of them.
fn blossom(power: usize, [a, b, c]: [f64; 3]) -> f64 {
    match power {
        0 => 1.0,
        1 => (a + b + c) / 3.0,
        2 => (a * b + a * c + b * c) / 3.0,
        _ => a * b * c,
    }
}

/// Clamped cubic knots over `[-1.5, 1.5]` in `spans` equal spans.
fn knots(spans: u32) -> Vec<f64> {
    let mut out = vec![-1.5; 3];
    out.extend((0..=spans).map(|k| -1.5 + 3.0 * f64::from(k) / f64::from(spans)));
    out.extend([1.5; 3]);
    out
}

/// The bicubic graph `z = Σ c[i][j] xⁱ yʲ` over `[-1.5, 1.5]²`, its first parameter `x`, or `-x` where `reversed`, in
/// `spans` spans each way, placed rigidly. Each pole is the blossom of the polynomial at its three knots, so the graph
/// is the polynomial exactly.
fn graph(c: &[[f64; 4]; 4], reversed: bool, spans: (u32, u32)) -> SplineSurface {
    let (ku, kv) = (knots(spans.0), knots(spans.1));
    let triple = |k: &[f64], i: u32| {
        let i = usize::try_from(i).unwrap();
        [k[i.saturating_add(1)], k[i.saturating_add(2)], k[i.saturating_add(3)]]
    };
    let (sign, mut poles) = (if reversed { -1.0 } else { 1.0 }, Vec::new());
    let count = (spans.0.saturating_add(3), spans.1.saturating_add(3));
    for j in 0..count.1 {
        for i in 0..count.0 {
            let (tu, tv) = (triple(&ku, i), triple(&kv, j));
            let mut z = 0.0;
            for (p, row) in c.iter().enumerate() {
                for (q, coefficient) in row.iter().enumerate() {
                    let flip = if reversed && p % 2 == 1 { -1.0 } else { 1.0 };
                    z += coefficient * flip * blossom(p, tu) * blossom(q, tv);
                }
            }
            poles.push(Point::from_vec3(placed(v(sign * blossom(1, tu), blossom(1, tv), z))));
        }
    }
    SplineSurface::new(&BsplineSurface { degree: (3, 3), count, knots_u: ku, knots_v: kv, poles, weights: None })
        .unwrap()
}

/// P5's `h = 0.3 x² − 0.2 x y + 0.1 y³`, the shape both graphs share.
fn shared() -> [[f64; 4]; 4] {
    let mut c = [[0.0; 4]; 4];
    c[2][0] = 0.3;
    c[1][1] = -0.2;
    c[0][3] = 0.1;
    c
}

/// `h` at `(x, y)`.
fn h(x: f64, y: f64) -> f64 {
    0.3 * x * x - 0.2 * x * y + 0.1 * y * y * y
}

/// P5's polynomial pair: `z = h + w` in three spans each way, and `z = h` read with `x` reversed in two spans by five.
fn pair(w: [[f64; 4]; 4]) -> (Arc<dyn Surface>, Arc<dyn Surface>) {
    let mut a = shared();
    for (row, add) in a.iter_mut().zip(w) {
        for (entry, more) in row.iter_mut().zip(add) {
            *entry += more;
        }
    }
    (Arc::new(graph(&a, false, (3, 3))), Arc::new(graph(&shared(), true, (2, 5))))
}

/// P5's `nt_poly_loop`: `w = (x − 0.37)² + (y + 0.41)² − ε`, a loop of radius `√ε` about `(0.37, −0.41)` on `h`.
fn poly_loop(eps: f64) -> (Arc<dyn Surface>, Arc<dyn Surface>) {
    let mut w = [[0.0; 4]; 4];
    (w[2][0], w[1][0], w[0][2], w[0][1]) = (1.0, -0.74, 1.0, 0.82);
    w[0][0] = 0.37 * 0.37 + 0.41 * 0.41 - eps;
    pair(w)
}

/// P5's `nt_poly_lines`: `w = (x − 0.2)² − ε`, the two curves `x = 0.2 ± √ε` across the square on `h`.
fn poly_lines(eps: f64) -> (Arc<dyn Surface>, Arc<dyn Surface>) {
    let mut w = [[0.0; 4]; 4];
    (w[2][0], w[1][0], w[0][0]) = (1.0, -0.4, 0.04 - eps);
    pair(w)
}

/// A case of the sweep: its name, its two surfaces, its true section as components of placed points, and the most
/// pieces it may come in.
struct Case {
    name: String,
    a: Arc<dyn Surface>,
    b: Arc<dyn Surface>,
    truth: Vec<Vec<Vec3>>,
    pieces: usize,
}

/// The sweep's two analytic families at the gap `eps`: two unit spheres `2 − ε` apart, meeting in a circle of radius
/// about `√ε`, and the plane `x = 1 − ε` against the unit cylinder about Z, in two lines `2 √(2 ε)` apart.
fn analytic(eps: f64) -> [Case; 2] {
    let apart = 2.0 - eps;
    let middle = apart / 2.0;
    let radius = ((1.0 - middle) * (1.0 + middle)).sqrt();
    let spheres = Case {
        name: format!("nt_ss_circle[{eps:e}]"),
        a: sphere(O, 1.0),
        b: sphere(v(apart, 0.0, 0.0), 1.0),
        truth: vec![circle(v(middle, 0.0, 0.0), X, radius, 400).into_iter().map(placed).collect()],
        pieces: 1,
    };
    let across = 1.0 - eps;
    let half = ((1.0 - across) * (1.0 + across)).sqrt();
    let line = |side: f64| (0..=60).map(|k| placed(v(across, side, -3.0 + 0.1 * f64::from(k)))).collect();
    let lines = Case {
        name: format!("nt_pc_two_lines[{eps:e}]"),
        a: plane(v(across, 0.0, 0.0), X),
        b: cylinder(O, Z, 1.0),
        truth: vec![line(half), line(-half)],
        pieces: 2,
    };
    [spheres, lines]
}

/// The sweep's four families at the gap `eps`: the two analytic ones, then P5's polynomial pairs, whose true section
/// is where `w` is nought, lifted onto `h`.
fn families(eps: f64) -> [Case; 4] {
    let [spheres, lines] = analytic(eps);
    let radius = eps.sqrt();
    let lift = |x: f64, y: f64| placed(v(x, y, h(x, y)));
    let around = (0..240).map(|k| {
        let (sine, cosine) = sin_cos(TAU * f64::from(k) / 240.0);
        lift(0.37 + radius * cosine, -0.41 + radius * sine)
    });
    let (a, b) = poly_loop(eps);
    let looped = Case { name: format!("nt_poly_loop[{eps:e}]"), a, b, truth: vec![around.collect()], pieces: 1 };
    let along = |x: f64| (0..=120).map(|k| lift(x, -1.5 + 0.025 * f64::from(k))).collect();
    let (a, b) = poly_lines(eps);
    let truth = vec![along(0.2 + radius), along(0.2 - radius)];
    let crossing = Case { name: format!("nt_poly_lines[{eps:e}]"), a, b, truth, pieces: 2 };
    [spheres, lines, looped, crossing]
}

/// The gaps of the sweep.
const GAPS: [f64; 12] = [1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12];

/// The closed forms, the pencil method and the hybrid, as `booleans` asks them, with P5's longest step, `0.03` of its
/// scale, and leaves a quarter of it, three times P5's, which the starts of second curves and of near touches make up
/// for, so that the sweep is quick. The tracer's sag is below the closest two curves the band tells apart here,
/// `2 √(2 band / 2)`. The tube about a curve found no longer depends on it, as a point inside it is confirmed on the
/// curve within the band, and the sweep passes at ten times this sag too.
fn methods() -> Methods {
    let tracer = Tracer { tol: tol(), sag: mm(1e-5), longest: mm(0.03 * SCALE), steps: 20_000 };
    let hybrid = Hybrid { tracer, leaf: mm(0.25 * SCALE) };
    Methods::new(vec![Box::new(Closed), Box::new(pencil()), Box::new(hybrid)])
}

/// The pencil method, within a box a thousand times the cases' scale about the origin, far past every part.
fn pencil() -> Pencil {
    Pencil::within(Bounds::around(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), mm(1e3 * SCALE)))
}

/// The section of `case`.
fn section(case: &Case) -> Section {
    methods().section(case.a.as_ref(), case.b.as_ref(), tol(), Budget { steps: 400_000 })
}

/// How a case came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// Every true component found by a curve, every curve on the true section, no more pieces than it may have, no
    /// contact and no failure.
    Correct,
    /// No curve or point off the true section, no failure, and every true point either on a curve or within the bound
    /// of a contact below the resolution.
    Below,
    /// Below the resolution, but a true point lies outside every bound.
    BoundMissed,
    /// Wrong, and a failure says so.
    Reported,
    /// Wrong, and nothing says so.
    Silent,
}

/// How near a curve must pass a true component, as P5 scored: the smaller of `2e-3` of the scale and a fifth of the
/// component's diameter.
fn near(component: &[Vec3]) -> f64 {
    let far = |p: &Vec3| component.iter().map(|q| (*p - *q).norm()).fold(0.0, |a: f64, b| if b > a { b } else { a });
    let wide = component.iter().map(far).fold(0.0, |a: f64, b| if b > a { b } else { a });
    let fifth = wide / 5.0;
    if fifth < 2e-3 * SCALE { fifth } else { 2e-3 * SCALE }
}

/// Points along `curve` where it runs beside `component`: across its range, or where that has no end, between the
/// parameters of its points nearest the component's ends.
fn along(curve: &dyn Curve, component: &[Vec3]) -> Vec<Vec3> {
    let range = curve.range();
    let (lo, hi) = if range.lo.is_finite() && range.hi.is_finite() {
        (range.lo, range.hi)
    } else {
        let at = |p: Vec3| curve.closest(Point::from_vec3(p), None, tol(), &mut Scratch::default()).unwrap().at;
        let (s, t) = (at(component[0]), at(*component.last().unwrap()));
        if s < t { (s, t) } else { (t, s) }
    };
    (0..=64).map(|k| curve.point(lo + (hi - lo) * f64::from(k) / 64.0).to_vec3()).collect()
}

/// How far `p` is from the nearest point of `truth`.
fn off(truth: &[Vec<Vec3>], p: Vec3) -> f64 {
    truth.iter().flatten().map(|q| (*q - p).norm()).fold(f64::INFINITY, smaller)
}

/// Whether a curve of `got` runs off `case`'s true section, by more than P5's tolerance and half the truth's spacing.
fn spurious(case: &Case, got: &Section) -> bool {
    let tol = case.truth.iter().map(|component| near(component)).fold(f64::INFINITY, smaller) + 0.05;
    let off_truth = |c: &SectionCurve| {
        let first = case.truth.first().map_or_default(Vec::as_slice);
        along(c.curve.as_ref(), first).iter().any(|p| off(&case.truth, *p) > tol)
    };
    got.curves.iter().any(off_truth)
}

/// The contacts of `got` below the resolution, as where and how far each reaches.
fn below(got: &Section) -> Vec<(Vec3, f64)> {
    let reach = |c: &Contact| match *c {
        Contact::BelowResolution { at, bound } => Some((at.to_vec3(), bound.millimetres())),
        Contact::Point { .. } => None,
    };
    got.contacts.iter().filter_map(reach).collect()
}

/// How `got` scores against `case`'s truth.
fn scored(case: &Case, got: &Section) -> Outcome {
    let found = |component: &Vec<Vec3>| {
        let tol = near(component);
        component.iter().all(|p| got.curves.iter().any(|c| from_curve(c.curve.as_ref(), *p) <= tol))
    };
    let points = got.contacts.iter().any(|c| matches!(c, Contact::Point { .. }));
    let clean = got.failures.is_empty() && !spurious(case, got) && !points;
    let below = below(got);
    let counted = !got.curves.is_empty() && got.curves.len() <= case.pieces;
    if clean && below.is_empty() && counted && case.truth.iter().all(found) {
        return Outcome::Correct;
    }
    if clean && !below.is_empty() {
        let held = |p: &Vec3| {
            below.iter().any(|(at, bound)| (*at - *p).norm() <= *bound)
                || got.curves.iter().any(|c| from_curve(c.curve.as_ref(), *p) <= 1e-9 * SCALE)
        };
        return if case.truth.iter().flatten().all(held) { Outcome::Below } else { Outcome::BoundMissed };
    }
    if got.failures.is_empty() { Outcome::Silent } else { Outcome::Reported }
}

/// The sweep's family `which`, each gap scored, every case correct or below the resolution with its bound holding.
fn sweep(which: usize) -> Vec<(String, Outcome)> {
    let mut out = Vec::new();
    for eps in GAPS {
        let case = families(eps).into_iter().nth(which).unwrap();
        let got = section(&case);
        out.push((case.name.clone(), scored(&case, &got)));
    }
    let wrong: Vec<&(String, Outcome)> =
        out.iter().filter(|(_, o)| !matches!(o, Outcome::Correct | Outcome::Below)).collect();
    assert!(wrong.is_empty(), "{wrong:?} in {out:?}");
    out
}

/// The names of the cases of `got` below the resolution.
fn named_below(got: &[(String, Outcome)]) -> Vec<&str> {
    got.iter().filter(|(_, o)| *o == Outcome::Below).map(|(name, _)| name.as_str()).collect()
}

#[test]
fn p5s_near_tangent_spheres_are_correct_then_below_the_resolution_and_never_silent() {
    let got = sweep(0);
    assert_eq!(named_below(&got), ["nt_ss_circle[1e-11]", "nt_ss_circle[1e-12]"], "the gaps within the band");
}

#[test]
fn p5s_near_tangent_plane_and_cylinder_are_correct_at_every_gap() {
    // Within the band, the two lines are one tangent line, which lies within P5's tolerance of both.
    let got = sweep(1);
    assert!(got.iter().all(|(_, o)| *o == Outcome::Correct), "{got:?}");
}

#[test]
fn p5s_near_tangent_polynomial_loop_is_correct_then_below_the_resolution_and_never_silent() {
    let got = sweep(2);
    assert_eq!(named_below(&got), ["nt_poly_loop[1e-10]", "nt_poly_loop[1e-11]", "nt_poly_loop[1e-12]"]);
}

#[test]
fn p5s_near_tangent_polynomial_lines_are_correct_then_below_the_resolution_and_never_silent() {
    let got = sweep(3);
    assert_eq!(named_below(&got), ["nt_poly_lines[1e-10]", "nt_poly_lines[1e-11]", "nt_poly_lines[1e-12]"]);
}

#[test]
fn nt_poly_loop_at_1e_10_p5s_one_silent_failure_is_below_the_resolution_with_a_bound_that_holds_its_loop() {
    // The loop is of radius 1e-5 about (0.37, -0.41): a gap of about 0.96e-10 across, against the band of 0.5e-10,
    // and too small for the trace, whose longest step takes its relative curvature to be 1 / 0.12.
    let [_, _, case, _] = families(1e-10);
    let got = section(&case);
    assert_eq!((got.method, got.curves.len(), got.failures.len()), (Some("hybrid"), 0, 0), "{:?}", got.failures);
    let [Contact::BelowResolution { at, bound }] = got.contacts.as_slice() else { panic!("{:?}", got.contacts) };
    let far = case.truth[0].iter().map(|p| (*p - at.to_vec3()).norm()).fold(0.0, |a: f64, b| if b > a { b } else { a });
    assert!(far <= bound.millimetres(), "the loop reaches {far} from it, beyond {bound:?}");
    assert!(bound.millimetres() <= 10.0 * far, "and the bound is near its size: {bound:?} against {far}");
}

/// The curves of `got` that pass within `near` of `p` other than at an end.
fn through(got: &Section, p: Vec3, near: f64) -> Vec<usize> {
    let mut out = Vec::new();
    for (k, c) in got.curves.iter().enumerate() {
        let range = c.curve.range();
        let found = c.curve.closest(Point::from_vec3(p), None, tol(), &mut Scratch::default()).unwrap();
        let inside = (found.at - range.lo).abs() > 1e-6 && (range.hi - found.at).abs() > 1e-6;
        if found.distance.millimetres() <= near && inside {
            out.push(k);
        }
    }
    out
}

/// How many ends of `got`'s curves are within `near` of `p`.
fn ends_at(got: &Section, p: Vec3, near: f64) -> usize {
    let ends = |c: &SectionCurve| {
        let range = c.curve.range();
        [range.lo, range.hi].into_iter().filter(|t| (c.curve.point(*t).to_vec3() - p).norm() <= near).count()
    };
    got.curves.iter().map(ends).sum()
}

#[test]
fn vivianis_node_cuts_its_curve_in_the_pencil_method_and_is_a_contact_below_the_resolution() {
    // The cylinder of radius 1 inside the sphere of radius 2, tangent at (2, 0, 0): the curve passes through the node
    // twice, and is cut there, every piece that comes to it ending there. The relative curvature is ±1/2.
    let (a, b) = (cylinder(X, Z, 1.0), sphere(O, 2.0));
    let both = Methods::new(vec![Box::new(Closed), Box::new(pencil())]);
    let got = both.section(a.as_ref(), b.as_ref(), tol(), Budget { steps: 16 });
    assert_eq!(got.method, Some("pencil"), "{:?}", got.failures);
    let node = placed(v(2.0, 0.0, 0.0));
    assert_eq!(through(&got, node, 1e-9), Vec::<usize>::new(), "no piece passes through the node");
    assert_eq!(ends_at(&got, node, 1e-9), 4, "four ends meet at the node");
    let [Contact::BelowResolution { at, bound }] = got.contacts.as_slice() else { panic!("{:?}", got.contacts) };
    let d = (2.0 * BAND / 0.5).sqrt();
    let (miss, slip) = ((at.to_vec3() - node).norm(), (bound.millimetres() - 2.0 * d).abs());
    assert!(miss <= 1e-9 && slip <= 1e-6 * d, "{at:?} {bound:?}");
}

#[test]
fn vivianis_node_on_a_spline_sphere_is_a_contact_where_the_hybrids_curves_end() {
    // P5's spline Viviani: the analytic cylinder inside a sphere of radius 2 as a rational B-spline, here turned about X
    // so that the curve passes neither of its poles. The node is kept as one contact, below the resolution, whose bound
    // holds the node and is at least twice `√(2 band / ½)` at the hybrid tests' band; and the curves of all four
    // branches end within it, cut there, none passing through.
    let band = 1e-7;
    let tol = Tolerance::new(Length::from_millimetres(band), Angle::from_radians(1e-9)).unwrap();
    let ball = Sphere::placed(&turned([1.0, 0.0, 0.0], 0.4, [0.0; 3]), mm(2.0)).unwrap();
    let spline = nurbs(&ball, (0.0, -FRAC_PI_2), (TAU, FRAC_PI_2));
    let tube = Cylinder::placed(&moved([1.0, 0.0, 0.0]), mm(1.0)).unwrap();
    let tracer = Tracer { tol, sag: mm(1e-4), longest: mm(0.05), steps: 20_000 };
    let methods = Methods::new(vec![Box::new(Hybrid { tracer, leaf: mm(0.25) })]);
    let got = methods.section(&tube, &spline, tol, Budget { steps: 400_000 });
    let node = Vec3::new(2.0, 0.0, 0.0);
    let [Contact::BelowResolution { at, bound }] = got.contacts.as_slice() else { panic!("{:?}", got.contacts) };
    let (d, reach) = ((2.0 * band / 0.5).sqrt(), bound.millimetres());
    assert!((at.to_vec3() - node).norm() <= reach && 2.0 * d <= reach && reach <= 4.0 * d, "{at:?} {bound:?}");
    assert!(ends_at(&got, at.to_vec3(), reach) >= 4, "the four branches end at it");
    assert_eq!(through(&got, node, 1e-6), Vec::<usize>::new(), "none passes through the node");
}
