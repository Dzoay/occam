//! Source: original
//! The hybrid against cases whose answers are known by construction: P5's many-loops surfaces against a plane, a curve
//! through a corner both subdivisions share, a rational cylinder on its analytic twin, a torus cut in two circles, a
//! sphere touching a plane, and what the budget and the depth cannot resolve; then the same sections at one and many
//! threads.

use std::f64::consts::TAU;
use std::num::NonZeroUsize;
use std::sync::Arc;

use occam_geom::{BsplineSurface, Content, Cylinder, Plane, Scratch, SplineSurface, Surface, Torus};
use occam_math::Exec;
use occam_report::{Code, Fault, NamedValue, Reason, Value};
use occam_units::{Angle, Direction, Placement, Point, UvPoint, Vec3, Vector, World};

use crate::decide::Decide;
use crate::on::On;
use crate::trace_tests::{BAND, ball, gap, level, mm, moved, nurbs_cylinder, point, tol, tracer, upright};
use crate::traverse::{DEPTH, held, traverse};
use crate::{Answer, Budget, Contact, Hybrid, Method, Methods, Section, SurfacePair, Work, sections};

/// A turn and a shift that align nothing with an axis, as P5 placed every case.
fn rigid() -> Placement<World, World> {
    let axis = Direction::normalised(Vec3::new(0.3, -0.5, 0.8));
    let spin = Placement::rotation(axis, Angle::from_radians(0.7));
    spin.then(&Placement::translation(Vector::from_vec3(Vec3::new(0.4, -1.3, 2.1))))
}

/// The point `(x, y, z)` placed rigidly.
fn placed(x: f64, y: f64, z: f64) -> Point<World> {
    rigid().point(Point::from_vec3(Vec3::new(x, y, z)))
}

/// A count as a number.
fn number(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap())
}

/// How many poles P5's many-loops surface for `l` has each way.
fn poles_for(l: usize) -> usize {
    l.saturating_mul(2).saturating_add(3)
}

/// The clamped uniform cubic knots of `count` poles: four at nought, one at each whole number up to `count - 4`, and
/// four at `count - 3`.
fn knots(count: usize) -> Vec<f64> {
    let spans = count.saturating_sub(3);
    let mut out = vec![0.0; 4];
    out.extend((1..spans).map(number));
    out.extend([number(spans); 4]);
    out
}

/// `sum a_i N_i(at)` over the knots of `count` poles, where `a_i` is `1` for even `i` and `-1` for odd, by de Boor's
/// algorithm written here apart from occam-geom's.
fn wave(at: f64, count: usize) -> f64 {
    let knot = knots(count);
    let span = (3..count).take_while(|&index| knot[index] <= at).last().unwrap_or(3);
    let first = span.saturating_sub(3);
    let sign = |index: usize| if index.is_multiple_of(2) { 1.0 } else { -1.0 };
    let mut rung: Vec<f64> = (first..=span).map(sign).collect();
    for level in 1..=3_usize {
        for slot in (level..=3).rev() {
            let index = first.saturating_add(slot);
            let width = knot[index.saturating_add(4).saturating_sub(level)] - knot[index];
            let along = if width > 0.0 { (at - knot[index]) / width } else { 0.0 };
            rung[slot] = (1.0 - along) * rung[slot.saturating_sub(1)] + along * rung[slot];
        }
    }
    rung[3]
}

/// The level P5 cut its many-loops surfaces at.
const CUT: f64 = 0.2;

/// P5's many-loops surface for `l`, placed rigidly, and the plane at the level it is cut: a bicubic B-spline of `2l + 3`
/// poles each way, the poles at a tenth of their Greville abscissae so that `x = u / 10` and `y = v / 10` exactly, and
/// at heights `a_i + a_j`. It is `wave(u) + wave(v)`, whose peaks inside the domain, at `u` near 1.1, then at each odd
/// number from 3 to `2l - 3`, then near `2l - 1.1`, are `l` each way, one in each two knot spans. The plane cuts a loop
/// about each of the `l^2` peaks, and one more about the whole domain, whose edges are all above it: `l^2 + 1` curves,
/// each closed (17, 145 and 577 for P5's surfaces of 25, 169 and 625 components, which it counted by cells).
fn many_loops(l: usize) -> (Plane, SplineSurface) {
    let count = poles_for(l);
    let knot = knots(count);
    let greville = |index: usize| knot[index.saturating_add(1)..index.saturating_add(4)].iter().sum::<f64>() / 3.0;
    let height = |index: usize| if index.is_multiple_of(2) { 1.0 } else { -1.0 };
    let poles = (0..count)
        .flat_map(|row| (0..count).map(move |column| (column, row)))
        .map(|(i, j)| placed(0.1 * greville(i), 0.1 * greville(j), height(i) + height(j)))
        .collect();
    let size = u32::try_from(count).unwrap();
    let form = BsplineSurface {
        degree: (3, 3),
        count: (size, size),
        knots_u: knot.clone(),
        knots_v: knot,
        poles,
        weights: None,
    };
    (Plane::placed(&moved([0.0, 0.0, CUT]).then(&rigid())), SplineSurface::new(&form).unwrap())
}

/// The hybrid with a tracer of `sag` and `longest` and leaves of `leaf`.
fn hybrid(sag: f64, longest: f64, leaf: f64) -> Hybrid {
    Hybrid { tracer: tracer(sag, longest, 20_000), leaf: mm(leaf) }
}

/// The hybrid alone, as the only method.
fn alone(method: Hybrid) -> Methods {
    Methods::new(vec![Box::new(method)])
}

/// A budget no case here comes near.
const PLENTY: Budget = Budget { steps: 50_000_000 };

/// The hybrid for the many-loops surfaces.
fn for_loops() -> Hybrid {
    hybrid(1e-4, 0.01, 0.1)
}

/// Checks the section of P5's many-loops surface for `l` against its answer: `l^2 + 1` closed curves, one about each
/// peak and one about the domain, each where `wave(u) + wave(v)` is the cut, with every decision decided.
fn the_loops_are_all_found(l: usize) {
    let (plane, spline) = many_loops(l);
    let found = alone(for_loops()).section(&plane, &spline, tol(), PLENTY);
    assert_eq!(found.method, Some("hybrid"));
    assert!(found.failures.is_empty(), "{:?}", found.failures);
    assert!(found.tally.decided > 0 && found.tally.undecided == 0, "{:?}", found.tally);
    let mut peaks = vec![0_u32; l.saturating_mul(l)];
    let mut around = 0_u32;
    for curve in &found.curves {
        let range = curve.curve.range();
        assert!(gap(curve.curve.point(range.lo), curve.curve.point(range.hi)) <= BAND, "every curve is closed");
        let samples: Vec<UvPoint> =
            (0..32).map(|k| curve.on_b.uv(range.lo + (range.hi - range.lo) * f64::from(k) / 32.0)).collect();
        for uv in &samples {
            let off = wave(uv.u(), poles_for(l)) + wave(uv.v(), poles_for(l)) - CUT;
            assert!(off.abs() <= 4.0 * BAND, "the curve is where the surface is at the cut: {off:e} at {uv:?}");
        }
        let extent = |pick: fn(UvPoint) -> f64| {
            let lo = samples.iter().map(|uv| pick(*uv)).fold(f64::INFINITY, |lo, x| if x < lo { x } else { lo });
            let hi = samples.iter().map(|uv| pick(*uv)).fold(f64::NEG_INFINITY, |hi, x| if x > hi { x } else { hi });
            hi - lo
        };
        let (wide, high) = (extent(UvPoint::u), extent(UvPoint::v));
        if wide > 2.0 * number(l) - 2.0 && high > 2.0 * number(l) - 2.0 {
            around = around.saturating_add(1);
            continue;
        }
        let first = samples[0];
        let (cell_u, cell_v) = (cell(first.u(), l), cell(first.v(), l));
        let stays = samples.iter().all(|uv| cell(uv.u(), l) == cell_u && cell(uv.v(), l) == cell_v);
        assert!(stays, "a loop about one peak stays in its two spans each way: {wide} by {high}");
        let slot = &mut peaks[cell_v.saturating_mul(l).saturating_add(cell_u)];
        *slot = slot.saturating_add(1);
    }
    assert_eq!(around, 1, "one curve about the whole domain");
    assert!(peaks.iter().all(|&n| n == 1), "one loop about each peak: {peaks:?}");
    assert_eq!(found.curves.len(), l.saturating_mul(l).saturating_add(1));
}

/// Which two knot spans of `l` a parameter is in.
fn cell(at: f64, l: usize) -> usize {
    (0..l).find(|&index| at < 2.0 * number(index.saturating_add(1))).unwrap_or(l)
}

#[test]
fn p5s_25_component_surface_against_a_plane_gives_all_17_curves() {
    the_loops_are_all_found(4);
}

#[test]
#[ignore = "P5's 625-component surface takes minutes in a debug build, so it runs nightly: `just hybrid-many-loops`"]
fn p5s_625_component_surface_against_a_plane_gives_all_577_curves() {
    the_loops_are_all_found(24);
}

/// The biquadratic graph over `[-1, 1]^2` of `ax x^2 + bx x + ay y^2 + by y`, placed rigidly: its parameters are `x`
/// and `y`, as the poles' Greville abscissae are.
fn graph((ax, bx): (f64, f64), (ay, by): (f64, f64)) -> SplineSurface {
    let side = |a: f64, b: f64| [a - b, -a, a + b];
    let (fx, fy) = (side(ax, bx), side(ay, by));
    let at = [-1.0, 0.0, 1.0];
    let poles = (0..3).flat_map(|j| (0..3).map(move |i| (i, j))).map(|(i, j)| placed(at[i], at[j], fx[i] + fy[j]));
    let knots = vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
    let (count, poles) = ((3, 3), poles.collect());
    SplineSurface::new(&BsplineSurface {
        degree: (2, 2),
        count,
        knots_u: knots.clone(),
        knots_v: knots,
        poles,
        weights: None,
    })
    .unwrap()
}

/// Two curved graphs that differ by `x - 0.7 y + 0.25 x^2 - 0.15 y^2`, which is nought along one curve across the
/// square, through the middle of both domains: a corner of every level of both subdivisions from the second on.
fn crossing_at_a_corner() -> (SplineSurface, SplineSurface) {
    (graph((0.3, 0.5), (0.2, -0.4)), graph((0.05, -0.5), (0.35, 0.3)))
}

/// Where the hybrid's start points on `a` and `b` are.
fn starts(a: &dyn Surface, b: &dyn Surface, method: Hybrid) -> Vec<Vec3> {
    let on = On::Here(a, b);
    let held = held(a, b, tol()).unwrap();
    let mut decide = Decide::new(BAND);
    let traversal = traverse(on, (held, method.leaf.millimetres()), &mut Work::of(PLENTY), &mut decide).unwrap();
    crate::hybrid::starts(on, held, tol(), traversal.leaves, &mut decide)
}

#[test]
fn a_curve_through_a_corner_both_subdivisions_share_is_found_once() {
    let (a, b) = crossing_at_a_corner();
    let method = hybrid(1e-4, 0.05, 0.2);
    let found = alone(method).section(&a, &b, tol(), PLENTY);
    assert!(found.failures.is_empty(), "{:?}", found.failures);
    assert!(found.tally.decided > 0 && found.tally.undecided == 0, "{:?}", found.tally);
    let [curve] = found.curves.as_slice() else { panic!("{} curves", found.curves.len()) };
    let range = curve.curve.range();
    let ends = [curve.on_a.uv(range.lo), curve.on_a.uv(range.hi)];
    let on_edge = |uv: &UvPoint| (uv.u().abs() - 1.0).abs() < 1e-6 || (uv.v().abs() - 1.0).abs() < 1e-6;
    assert!(ends.iter().all(on_edge), "from edge to edge: {ends:?}");
    let corner = placed(0.0, 0.0, 0.0);
    let near = curve.curve.closest(corner, None, tol(), &mut Scratch::default()).unwrap().distance.millimetres();
    assert!(near <= BAND, "through the corner: {near:e}");
    let around =
        starts(&a, &b, method).iter().filter(|at| (**at - corner.to_vec3()).norm() < method.leaf.millimetres()).count();
    assert!(around > 1, "the leaves about the corner each gave a start on it: {around}");
}

#[test]
fn a_rational_cylinder_on_its_analytic_twin_is_an_unresolved_overlap_with_no_curves() {
    let twin = Cylinder::placed(&moved([0.0; 3]), mm(2.0)).unwrap();
    let rational = nurbs_cylinder((0.0, TAU));
    let found = alone(hybrid(1e-4, 0.05, 0.25)).section(&twin, &rational, tol(), PLENTY);
    assert_eq!(found.method, Some("hybrid"));
    assert!(found.curves.is_empty() && found.contacts.is_empty(), "{} curves", found.curves.len());
    let [report] = found.failures.as_slice() else { panic!("{:?}", found.failures) };
    let kind = (report.code, report.fault, report.reason);
    assert_eq!(kind, (Code::Incomplete, Some(Fault::NotImplemented), Some(Reason::SurfacesCoincide)));
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    assert_eq!((value("curves"), value("contacts")), (Some(0.0), Some(0.0)));
    assert!(value("leaves").is_some_and(|n| n > 16.0), "{:?}", value("leaves"));
    let Some(Value::Point([x, y, z])) = report.value("at").map(|v| v.value) else { panic!() };
    assert!(((x * x + y * y).sqrt() - 2.0).abs() < 1e-9 && z.abs() <= 1.0, "on both: {x} {y} {z}");
    assert_eq!(report.remedies, []);
    assert!(found.tally.decided > 0 && found.tally.undecided == 0, "{:?}", found.tally);
}

#[test]
fn a_torus_is_subdivided_against_a_planes_implicit_form_where_the_plane_has_no_ends() {
    // They meet in two circles about the axis, of radii 3 - sqrt(0.75) and 3 + sqrt(0.75).
    let torus = Torus::placed(&moved([0.0; 3]), mm(3.0), mm(1.0)).unwrap();
    let found = alone(hybrid(1e-5, 0.1, 0.5)).section(&level(0.5), &torus, tol(), PLENTY);
    assert!(found.failures.is_empty(), "{:?}", found.failures);
    let radius = |p: Point<World>| {
        let v = p.to_vec3();
        (v.x * v.x + v.y * v.y).sqrt()
    };
    let mut radii: Vec<f64> = found.curves.iter().map(|c| radius(c.curve.point(c.curve.range().lo))).collect();
    radii.sort_by(f64::total_cmp);
    let want = [3.0 - 0.75_f64.sqrt(), 3.0 + 0.75_f64.sqrt()];
    assert_eq!(radii.len(), 2, "{radii:?}");
    assert!(radii.iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-6), "{radii:?}");
}

#[test]
fn a_sphere_touching_a_plane_is_one_contact() {
    let (plane, sphere) = (upright(0.0), ball(2.0, [2.0, 0.0, 0.0]));
    let found = alone(hybrid(1e-4, 0.05, 0.25)).section(&plane, &sphere, tol(), PLENTY);
    assert!(found.curves.is_empty() && found.failures.is_empty(), "{} {:?}", found.curves.len(), found.failures);
    let [Contact::Point { at, .. }] = found.contacts.as_slice() else { panic!("{:?}", found.contacts) };
    assert!(at.to_vec3().norm() < 1e-6, "where they touch: {at:?}");
}

#[test]
fn a_subdivision_the_budget_cannot_pay_for_is_reported_with_nothing_guessed() {
    let (plane, spline) = many_loops(4);
    let found = alone(for_loops()).section(&plane, &spline, tol(), Budget { steps: 200 });
    assert_eq!(found.method, Some("hybrid"));
    let [report] = found.failures.as_slice() else { panic!("{:?}", found.failures) };
    assert_eq!((report.code, report.reason), (Code::Incomplete, Some(Reason::BudgetSpent)));
    assert_eq!(report.value("budget").and_then(NamedValue::number), Some(200.0));
    assert!(found.curves.is_empty() && found.tally.decided > 0, "{:?}", found.tally);
}

/// The bilinear patch over `[0, 1]^2` with corners at heights 0, 0.5, 0.5 and 1.2, placed rigidly: above the plane
/// `z = 0` but at its corner at the origin, which touches it.
fn touching_at_a_corner() -> (Plane, SplineSurface) {
    let poles = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.5), (0.0, 1.0, 0.5), (1.0, 1.0, 1.2)].map(|(x, y, z)| placed(x, y, z));
    let knots = vec![0.0, 0.0, 1.0, 1.0];
    let form = BsplineSurface {
        degree: (1, 1),
        count: (2, 2),
        knots_u: knots.clone(),
        knots_v: knots,
        poles: poles.into(),
        weights: None,
    };
    (Plane::placed(&moved([0.0; 3]).then(&rigid())), SplineSurface::new(&form).unwrap())
}

#[test]
fn parts_still_larger_than_a_leaf_when_halved_as_far_as_they_go_are_reported_unresolved() {
    // Only the parts at the corner come within the band of the plane, and none halved 48 times is as small as these
    // leaves.
    let (plane, patch) = touching_at_a_corner();
    let found = alone(hybrid(1e-4, 0.05, 1e-12)).section(&plane, &patch, tol(), PLENTY);
    assert!(found.curves.is_empty() && found.contacts.is_empty());
    let [report] = found.failures.as_slice() else { panic!("{:?}", found.failures) };
    let kind = (report.code, report.fault, report.reason);
    assert_eq!(kind, (Code::Incomplete, Some(Fault::NumericallySensitive), Some(Reason::BudgetSpent)));
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    assert_eq!(value("depth"), Some(f64::from(DEPTH)));
    assert!(value("leaves").is_some_and(|n| n >= 1.0), "{:?}", value("leaves"));
    let Some(Value::Point([x, y, z])) = report.value("at").map(|v| v.value) else { panic!() };
    assert!(gap(point(x, y, z), placed(0.0, 0.0, 0.0)) < 1e-6, "at the corner: {x} {y} {z}");
    assert_eq!(report.remedies, []);
    assert!(found.tally.decided > 0 && found.tally.undecided == 0, "{:?}", found.tally);
}

#[test]
fn a_pair_with_no_surface_it_can_subdivide_is_declined() {
    let (a, b) = (level(0.0), upright(0.0));
    let mut section = Section::default();
    let answer = for_loops().attempt(&a, &b, tol(), &mut Work::of(PLENTY), &mut section);
    assert_eq!(answer.ok(), Some(Answer::Declined));
}

/// What a section gives, as text: each curve's content and its images', the contacts, the failures and the tally.
fn digest(found: &Section) -> Vec<String> {
    let words = |content: &mut Content| format!("{:?}", content.words());
    let mut out = Vec::new();
    for curve in &found.curves {
        let (mut c, mut a, mut b) = (Content::default(), Content::default(), Content::default());
        curve.curve.content(&mut c);
        curve.on_a.content(&mut a);
        curve.on_b.content(&mut b);
        out.extend([words(&mut c), words(&mut a), words(&mut b)]);
    }
    out.push(format!("{:?} {:?} {:?} {:?}", found.method, found.contacts, found.failures, found.tally));
    out
}

#[test]
fn sections_are_the_same_bits_at_one_and_many_threads() {
    let (plane, spline) = many_loops(2);
    let (a, b) = crossing_at_a_corner();
    let twin: Arc<dyn Surface> = Arc::new(Cylinder::placed(&moved([0.0; 3]), mm(2.0)).unwrap());
    let pairs: Vec<SurfacePair> = vec![
        (Arc::new(plane), Arc::new(spline)),
        (Arc::new(a), Arc::new(b)),
        (twin, Arc::new(nurbs_cylinder((0.0, TAU)))),
    ];
    let (methods, pairs): (_, Arc<[SurfacePair]>) = (Arc::new(alone(for_loops())), pairs.into());
    let one = sections(&Exec::serial(), Arc::clone(&methods), Arc::clone(&pairs), tol(), PLENTY);
    let many = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let other = sections(&many, Arc::clone(&methods), Arc::clone(&pairs), tol(), PLENTY);
    assert_eq!(one.len(), 3);
    assert!(
        one.iter().all(|found| found.method == Some("hybrid")) && one[0].curves.len() == 5 && one[1].curves.len() == 1
    );
    for ((alone, together), (a, b)) in one.iter().zip(&other).zip(pairs.iter()) {
        assert_eq!(digest(together), digest(alone), "{} threads", many.thread_count());
        assert_eq!(digest(&methods.section(a.as_ref(), b.as_ref(), tol(), PLENTY)), digest(alone), "on this thread");
    }
}
