//! Source: original
//! The hits of a curve on a surface: crossings and a contact on a cylinder, an overlap in a plane, an end on a sphere,
//! a surface with no analytic form, a spent budget, and the box test compared with the unculled scan by digest on a
//! generated set of primitives placed as the corpus's families place them.

pub(crate) use occam_fixtures::{mm, tol};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::num::NonZeroUsize;
use std::sync::Arc;

use occam_geom::{
    BsplineCurve, BsplineSurface, Circle, Cone, Content, Curve, Cylinder, FamilyName, Line, ParamRange,
    ParametricCurve, Plane, Scratch, Shape, Sphere, SplineSurface, Surface, Torus, UvBox,
};
use occam_math::{Digest, Exec, Real, atan2};
use occam_report::{Code, NamedValue, Reason, Report};
use occam_units::{Angle, Bounds, Direction, Frame, Local, Placement, Point, Tangent, UvPoint, Vec3, Vector, World};

use crate::{Budget, Hit, HitPair, Hits, Overlap, Span, boxes_meet, hits, hits_of_pairs, scan};

pub(crate) const BUDGET: Budget = Budget { steps: 100_000 };

pub(crate) fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Turned by `turn` about `axis`, then moved by `shift`, within one frame.
pub(crate) fn motion<F: Frame>(axis: Vec3, turn: f64, shift: Vec3) -> Placement<F, F> {
    let spin = Placement::rotation(Direction::normalised(axis), Angle::from_radians(turn));
    spin.then(&Placement::translation(Vector::from_vec3(shift)))
}

/// A frame in the world: [`motion`] of the world's.
pub(crate) fn frame(axis: Vec3, turn: f64, shift: Vec3) -> Placement<Local, World> {
    Placement::identity().then(&motion(axis, turn, shift))
}

/// A frame within `p`: [`motion`] of its own, then `p`.
pub(crate) fn within(p: &Placement<Local, World>, axis: Vec3, turn: f64, shift: Vec3) -> Placement<Local, World> {
    motion::<Local>(axis, turn, shift).then(p)
}

/// The placements every case is tried under: none, and two rigid motions.
pub(crate) fn placements() -> [Placement<Local, World>; 3] {
    [
        Placement::identity(),
        frame(v(1.0, 2.0, 3.0), 0.7, v(10.0, -4.0, 2.5)),
        frame(v(-0.3, 0.1, 1.0), 2.9, v(-7.0, 3.0, 11.0)),
    ]
}

/// The segment from local `from` along local `along`, over `0..=length`.
pub(crate) fn segment(p: &Placement<Local, World>, from: Vec3, along: Vec3, length: f64) -> Arc<dyn Curve> {
    let start = p.point(Point::from_vec3(from));
    Arc::new(Line::new(start, p.direction(Direction::normalised(along))).trimmed(0.0, length).unwrap())
}

pub(crate) fn span(lo: f64, hi: f64) -> Span {
    Span { lo, hi }
}

pub(crate) fn patch(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
    UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1))
}

/// The distance between two points.
pub(crate) fn apart(a: Point<World>, b: Point<World>) -> f64 {
    (a.to_vec3() - b.to_vec3()).norm()
}

/// That `found` is a hit at the curve's parameter `t`, its point on `surface` at its parameters.
pub(crate) fn hit_at(found: &Hit, t: f64, surface: &dyn Surface) {
    assert!((found.t - t).abs() < 1e-9, "{} against {t}", found.t);
    assert!(apart(found.at, surface.point(found.uv)) < 1e-9, "{found:?}");
}

/// That a scan met no failure, and found `counts` crossings, contacts and overlaps.
pub(crate) fn found(hits: &Hits, counts: (usize, usize, usize)) {
    assert!(hits.failures.is_empty(), "{:?}", hits.failures);
    assert_eq!((hits.crossings.len(), hits.contacts.len(), hits.overlaps.len()), counts, "{hits:?}");
}

#[test]
fn a_line_through_a_cylinder_gives_two_crossings() {
    for p in placements() {
        let cylinder = Cylinder::placed(&p, mm(2.0)).unwrap();
        let line = segment(&p, v(-5.0, 0.5, 1.0), v(1.0, 0.0, 0.0), 10.0);
        let got = hits(line.as_ref(), span(0.0, 10.0), &cylinder, patch((0.0, -10.0), (TAU, 10.0)), tol(), BUDGET);
        found(&got, (2, 0, 0));
        let x = (4.0_f64 - 0.25).sqrt();
        for (hit, (t, u)) in got.crossings.iter().zip([(5.0 - x, atan2(0.5, -x)), (5.0 + x, atan2(0.5, x))]) {
            hit_at(hit, t, &cylinder);
            assert!((hit.uv.u() - u).abs() < 1e-9 && (hit.uv.v() - 1.0).abs() < 1e-9, "{hit:?}");
        }
        assert!(got.tally.decided > 0 && got.tally.undecided == 0, "{:?}", got.tally);
    }
}

#[test]
fn a_tangent_line_gives_one_contact_and_a_near_one_none_or_two_crossings() {
    for p in placements() {
        let cylinder = Cylinder::placed(&p, mm(2.0)).unwrap();
        let side = patch((0.0, -10.0), (TAU, 10.0));
        let at = |y: f64| {
            let line = segment(&p, v(-5.0, y, 1.0), v(1.0, 0.0, 0.0), 10.0);
            hits(line.as_ref(), span(0.0, 10.0), &cylinder, side, tol(), BUDGET)
        };
        let touch = at(2.0);
        found(&touch, (0, 1, 0));
        hit_at(&touch.contacts[0], 5.0, &cylinder);
        assert!((touch.contacts[0].uv.u() - FRAC_PI_2).abs() < 1e-6, "{:?}", touch.contacts);
        found(&at(2.0 + 1e-6), (0, 0, 0));
        let cut = at(2.0 - 1e-6);
        found(&cut, (2, 0, 0));
        let x = (4.0_f64 - (2.0 - 1e-6) * (2.0 - 1e-6)).sqrt();
        hit_at(&cut.crossings[0], 5.0 - x, &cylinder);
        hit_at(&cut.crossings[1], 5.0 + x, &cylinder);
    }
}

#[test]
fn a_circle_lying_in_a_plane_is_an_overlap() {
    for p in placements() {
        let plane = Plane::placed(&p);
        let circle = |lift: f64, tilt: f64| {
            let circle = Circle::placed(&within(&p, v(1.0, 0.0, 0.0), tilt, v(1.0, -2.0, lift)), mm(3.0)).unwrap();
            hits(&circle, span(0.0, TAU), &plane, patch((-10.0, -10.0), (10.0, 10.0)), tol(), BUDGET)
        };
        for lift in [0.0, 5e-8] {
            let lying = circle(lift, 0.0);
            found(&lying, (0, 0, 1));
            assert_eq!(lying.overlaps, [Overlap { lo: 0.0, hi: TAU }], "lifted {lift}");
        }
        found(&circle(1e-3, 0.0), (0, 0, 0));
        let tilted = circle(1.0, 0.5);
        found(&tilted, (2, 0, 0));
        for hit in &tilted.crossings {
            assert!(apart(hit.at, plane.point(hit.uv)) < 1e-9, "{hit:?}");
        }
    }
}

#[test]
fn a_curve_wholly_within_the_band_lies_in_the_surface_however_it_bends_or_tilts() {
    // Half a band inside a cylinder of radius 1, and so long that it bends 1.4 bands across its length.
    let band = tol().band().millimetres();
    let length = (8.0 * 1.4 * band).sqrt();
    let line =
        Line::new(Point::from_vec3(v(-0.5 * length, 1.0 - 0.5 * band, 0.0)), Direction::normalised(v(1.0, 0.0, 0.0)));
    let cylinder = Cylinder::placed(&Placement::identity(), mm(1.0)).unwrap();
    let side = patch((0.0, -1.0), (TAU, 1.0));
    let short = hits(&line.trimmed(0.0, length).unwrap(), span(0.0, length), &cylinder, side, tol(), BUDGET);
    found(&short, (0, 0, 1));
    assert_eq!(short.overlaps, [Overlap { lo: 0.0, hi: length }]);
    // A circle of radius 5 tilted from a plane so little, its sine the angle itself, that it is never more than 0.6
    // bands from it.
    let tilt = 0.6 * band / 5.0;
    let circle = Circle::placed(&frame(v(1.0, 0.0, 0.0), tilt, v(1.0, -2.0, 0.0)), mm(5.0)).unwrap();
    let plane = Plane::placed(&Placement::identity());
    let tilted = hits(&circle, span(0.0, TAU), &plane, patch((-10.0, -10.0), (10.0, 10.0)), tol(), BUDGET);
    found(&tilted, (0, 0, 1));
    assert_eq!(tilted.overlaps, [Overlap { lo: 0.0, hi: TAU }]);
}

#[test]
fn a_segment_ending_on_a_sphere_gives_a_contact_at_its_end_and_a_patch_keeps_only_its_own() {
    let (ball, origin) = (Sphere::placed(&Placement::identity(), mm(2.0)).unwrap(), Placement::identity());
    let whole = patch((0.0, -FRAC_PI_2), (TAU, FRAC_PI_2));
    let inward = segment(&origin, v(5.0, 0.0, 0.0), v(-1.0, 0.0, 0.0), 3.0);
    let ending = hits(inward.as_ref(), span(0.0, 3.0), &ball, whole, tol(), BUDGET);
    found(&ending, (0, 1, 0));
    hit_at(&ending.contacts[0], 3.0, &ball);
    let through = segment(&origin, v(5.0, 0.0, 0.5), v(-1.0, 0.0, 0.0), 10.0);
    let both = hits(through.as_ref(), span(0.0, 10.0), &ball, whole, tol(), BUDGET);
    found(&both, (2, 0, 0));
    let east = patch((-FRAC_PI_2, -1.0), (FRAC_PI_2, 1.0));
    let one = hits(through.as_ref(), span(0.0, 10.0), &ball, east, tol(), BUDGET);
    found(&one, (1, 0, 0));
    assert_eq!(one.crossings[0].t, both.crossings[0].t, "only the crossing at positive x has its foot in the patch");
    assert!(one.crossings[0].uv.u() < 0.5, "the foot's turn is moved into the patch: {:?}", one.crossings);
}

/// The hyperbolic paraboloid `z = x y` over `[-1, 1]²`, as a bilinear B-spline: it presents no analytic form.
pub(crate) fn saddle() -> SplineSurface {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];
    let poles = corners.map(|(x, y)| Point::from_vec3(v(x, y, x * y))).to_vec();
    let knots = vec![0.0, 0.0, 1.0, 1.0];
    let form =
        BsplineSurface { degree: (1, 1), count: (2, 2), knots_u: knots.clone(), knots_v: knots, poles, weights: None };
    SplineSurface::new(&form).unwrap()
}

#[test]
fn a_surface_with_no_analytic_form_is_subdivided_and_newton_runs_on_the_leaves() {
    let (surface, unit, origin) = (saddle(), patch((0.0, 0.0), (1.0, 1.0)), Placement::identity());
    assert!(surface.analytic(tol()).is_none());
    let down = segment(&origin, v(0.5, 0.25, 2.0), v(0.0, 0.0, -1.0), 4.0);
    let through = hits(down.as_ref(), span(0.0, 4.0), &surface, unit, tol(), BUDGET);
    found(&through, (1, 0, 0));
    hit_at(&through.crossings[0], 2.0 - 0.125, &surface);
    assert!(apart(through.crossings[0].at, Point::from_vec3(v(0.5, 0.25, 0.125))) < 1e-9);
    found(&hits(down.as_ref(), span(0.0, 2.0 - 0.125), &surface, unit, tol(), BUDGET), (0, 1, 0));
    let beside = segment(&origin, v(3.0, 0.25, 2.0), v(0.0, 0.0, -1.0), 4.0);
    found(&scan(beside.as_ref(), span(0.0, 4.0), &surface, unit, tol(), BUDGET), (0, 0, 0));
}

#[test]
fn a_spent_budget_is_reported_and_keeps_what_was_found_before_it() {
    let p = Placement::identity();
    let cylinder = Cylinder::placed(&p, mm(2.0)).unwrap();
    let line = segment(&p, v(-5.0, 0.5, 1.0), v(1.0, 0.0, 0.0), 10.0);
    let run = |steps: u32| {
        hits(line.as_ref(), span(0.0, 10.0), &cylinder, patch((0.0, -10.0), (TAU, 10.0)), tol(), Budget { steps })
    };
    let whole = run(BUDGET.steps);
    let steps = (1..2000).find(|&steps| run(steps).crossings.len() == 1).unwrap();
    let part = run(steps);
    assert_eq!(part.crossings[..], whole.crossings[..1], "the first crossing is kept");
    let [report] = part.failures.as_slice() else { panic!("{:?}", part.failures) };
    assert_eq!((report.code, report.reason), (Code::HitsIncomplete, Some(Reason::BudgetSpent)));
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    assert_eq!((value("hits"), value("overlaps"), value("budget")), (Some(1.0), Some(0.0), Some(f64::from(steps))));
    assert!(report.remedies.is_empty(), "no remedy: an agent cannot set a budget");
}

/// Everything found, every number by its bits, and every failure, but not the tally: a culled pair makes no decisions.
fn digest(hits: &Hits) -> u64 {
    let mut out = Digest::EMPTY;
    for (tag, list) in [(1, &hits.crossings), (2, &hits.contacts)] {
        out.write_u64(tag);
        for hit in list {
            let at = hit.at.to_vec3();
            for x in [hit.t, at.x, at.y, at.z, hit.uv.u(), hit.uv.v()] {
                out.write_value(x);
            }
        }
    }
    for overlap in &hits.overlaps {
        out.write_value(overlap.lo);
        out.write_value(overlap.hi);
    }
    out.write_bytes(format!("{:?}", hits.failures).as_bytes());
    out.finish()
}

/// A primitive's faces, each a surface over a patch, and its edges, each a curve over a span.
type Faces = Vec<(Arc<dyn Surface>, UvBox)>;
type Edges = Vec<(Arc<dyn Curve>, Span)>;

/// A box of `size` with a corner at the origin of `p`: six planes and twelve lines.
fn cuboid(p: &Placement<Local, World>, size: Vec3) -> (Faces, Edges) {
    let (x, y, z, none) = (v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0), v(0.0, 0.0, 0.0));
    let sides = [
        (z, 0.0, none, (size.x, size.y)),
        (z, 0.0, z * size.z, (size.x, size.y)),
        (x, FRAC_PI_2, none, (size.x, size.z)),
        (x, FRAC_PI_2, y * size.y, (size.x, size.z)),
        (y, -FRAC_PI_2, none, (size.z, size.y)),
        (y, -FRAC_PI_2, x * size.x, (size.z, size.y)),
    ];
    let faces = sides.map(|(axis, turn, shift, far)| -> (Arc<dyn Surface>, UvBox) {
        (Arc::new(Plane::placed(&within(p, axis, turn, shift))), patch((0.0, 0.0), far))
    });
    let mut edges = Vec::new();
    for (along, length, across) in [
        (x, size.x, (y * size.y, z * size.z)),
        (y, size.y, (x * size.x, z * size.z)),
        (z, size.z, (x * size.x, y * size.y)),
    ] {
        for from in [none, across.0, across.1, across.0 + across.1] {
            edges.push((segment(p, from, along, length), span(0.0, length)));
        }
    }
    (faces.to_vec(), edges)
}

/// A cylinder of `radius` and `height` about the Z axis of `p`, standing on its XY plane, with its two caps, their
/// circles and its seam.
fn drum(p: &Placement<Local, World>, radius: f64, height: f64) -> (Faces, Edges) {
    let wall: Arc<dyn Surface> = Arc::new(Cylinder::placed(p, mm(radius)).unwrap());
    let (mut faces, mut edges): (Faces, Edges) = (vec![(wall, patch((0.0, 0.0), (TAU, height)))], Vec::new());
    edges.push((segment(p, v(radius, 0.0, 0.0), v(0.0, 0.0, 1.0), height), span(0.0, height)));
    for lift in [0.0, height] {
        let cap = within(p, v(0.0, 0.0, 1.0), 0.0, v(0.0, 0.0, lift));
        faces.push((Arc::new(Plane::placed(&cap)), patch((-radius, -radius), (radius, radius))));
        edges.push((Arc::new(Circle::placed(&cap, mm(radius)).unwrap()), span(0.0, TAU)));
    }
    (faces, edges)
}

/// A sphere of radius 3, a cone from a circle of radius 3, or a torus of radii 4 and 1, about `p`, with a meridian or
/// a circle of it.
fn round(p: &Placement<Local, World>, which: u8) -> (Faces, Edges) {
    let meridian = |radius: f64, centre: f64, lo: f64, hi: f64| -> (Arc<dyn Curve>, Span) {
        let upright = within(p, v(1.0, 0.0, 0.0), FRAC_PI_2, v(centre, 0.0, 0.0));
        (Arc::new(Circle::placed(&upright, mm(radius)).unwrap()), span(lo, hi))
    };
    let (face, edge): (Arc<dyn Surface>, _) = match which {
        0 => (Arc::new(Sphere::placed(p, mm(3.0)).unwrap()), meridian(3.0, 0.0, 0.0, PI)),
        1 => (Arc::new(Cone::placed(p, mm(3.0), Angle::from_radians(0.4)).unwrap()), meridian(1.0, 6.0, 0.0, TAU)),
        _ => (Arc::new(Torus::placed(p, mm(4.0), mm(1.0)).unwrap()), meridian(1.0, 4.0, 0.0, TAU)),
    };
    let around = if which == 0 {
        patch((0.0, -FRAC_PI_2), (TAU, FRAC_PI_2))
    } else {
        patch((0.0, 0.0), (TAU, if which == 1 { 4.0 } else { TAU }))
    };
    (vec![(face, around)], vec![edge])
}

/// Pairs of solids placed as the corpus's families place theirs: boxes overlapping at a corner, a slab sharing a face,
/// stacked boxes, coaxial and crossed cylinders, a plate with a hole, a sphere through a box, a cone and a torus
/// against cylinders, and a saddle in a box: every edge of each against every face of the other.
fn generated() -> Vec<HitPair> {
    let at = |x: f64, y: f64, z: f64| frame(v(0.0, 0.0, 1.0), 0.0, v(x, y, z));
    let crossed = frame(v(1.0, 0.0, 0.0), FRAC_PI_2, v(0.0, 5.0, 2.0));
    let saddle: (Faces, Edges) = (vec![(Arc::new(saddle()), patch((0.0, 0.0), (1.0, 1.0)))], Vec::new());
    let cases = [
        (cuboid(&at(0.0, 0.0, 0.0), v(4.0, 4.0, 4.0)), cuboid(&at(2.0, 2.0, 2.0), v(4.0, 4.0, 4.0))),
        (cuboid(&at(0.0, 0.0, 0.0), v(6.0, 6.0, 2.0)), cuboid(&at(1.0, 1.0, 2.0), v(3.0, 3.0, 1.0))),
        (cuboid(&at(0.0, 0.0, 0.0), v(4.0, 4.0, 4.0)), cuboid(&at(0.0, 0.0, 4.0), v(4.0, 4.0, 4.0))),
        (drum(&at(0.0, 0.0, 0.0), 2.0, 6.0), drum(&at(0.0, 0.0, 2.0), 2.0, 6.0)),
        (drum(&at(0.0, 0.0, -5.0), 2.0, 10.0), drum(&crossed, 2.0, 10.0)),
        (cuboid(&at(-5.0, -5.0, 0.0), v(10.0, 10.0, 2.0)), drum(&at(0.5, 0.5, -1.0), 1.5, 4.0)),
        (cuboid(&at(0.0, 0.0, 0.0), v(4.0, 4.0, 4.0)), round(&at(4.0, 2.0, 2.0), 0)),
        (drum(&at(0.0, 0.0, -1.0), 2.5, 5.0), round(&at(0.0, 0.0, 0.0), 1)),
        (drum(&at(4.0, 0.0, -3.0), 1.0, 6.0), round(&at(0.0, 0.0, 0.0), 2)),
        (cuboid(&at(-0.5, -0.5, -0.5), v(1.0, 1.0, 1.0)), saddle),
    ];
    let mut pairs = Vec::new();
    for (a, b) in &cases {
        for (edges, faces) in [(&a.1, &b.0), (&b.1, &a.0)] {
            for ((curve, span), (surface, patch)) in edges.iter().flat_map(|e| faces.iter().map(move |f| (e, f))) {
                pairs.push(HitPair {
                    curve: Arc::clone(curve),
                    span: *span,
                    surface: Arc::clone(surface),
                    patch: *patch,
                });
            }
        }
    }
    pairs
}

#[test]
fn on_a_generated_set_the_box_test_removes_scans_and_changes_no_digest() {
    let pairs = generated();
    let (mut culled, mut points) = (0, 0);
    for (i, pair) in pairs.iter().enumerate() {
        let (curve, surface) = (pair.curve.as_ref(), pair.surface.as_ref());
        let unculled = scan(curve, pair.span, surface, pair.patch, tol(), BUDGET);
        assert!(unculled.failures.is_empty(), "pair {i}: {:?}", unculled.failures);
        let boxed = hits(curve, pair.span, surface, pair.patch, tol(), BUDGET);
        assert_eq!(digest(&boxed), digest(&unculled), "pair {i}: {boxed:?} against {unculled:?}");
        culled += usize::from(!boxes_meet(curve, pair.span, surface, pair.patch, tol()));
        points += unculled.crossings.len() + unculled.contacts.len() + unculled.overlaps.len();
    }
    assert!(culled * 2 > pairs.len(), "the box test removes {culled} of {} scans", pairs.len());
    assert!(points > 50, "{points} hits, so the digests compare something");
}

#[test]
fn hits_are_the_same_bits_at_one_and_many_threads() {
    let pairs: Arc<[HitPair]> = generated().into();
    let alone: Vec<u64> =
        hits_of_pairs(&Exec::serial(), Arc::clone(&pairs), tol(), BUDGET).iter().map(digest).collect();
    assert_eq!(alone.len(), pairs.len());
    let distinct: std::collections::BTreeSet<u64> = alone.iter().copied().collect();
    assert!(distinct.len() > 40, "{} distinct results, so order shows", distinct.len());
    let many = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let got: Vec<u64> = hits_of_pairs(&many, pairs, tol(), BUDGET).iter().map(digest).collect();
    assert_eq!(got, alone);
}

/// How a [`Seeded`] line's boxes and B-spline form are seeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seed {
    /// Its box is the box of its ends, exact where it is lowest, rising from its start.
    Exact,
    /// That box one ulp too small: its low Z one double up.
    Short,
    /// Its box is the whole line's over any part, as a B-spline's is inside a knot span, and its B-spline form over a
    /// part runs over other parameters: the stretch half the part further on.
    Lying,
}

/// A line whose boxes and B-spline form are seeded.
#[derive(Clone, Copy)]
struct Seeded {
    line: Line,
    seed: Seed,
}

impl ParametricCurve for Seeded {
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        self.line.eval_point(t)
    }

    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        self.line.eval_first(t)
    }

    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        self.line.eval_second(t)
    }

    fn derivatives(
        &self,
        t: f64,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        ParametricCurve::derivatives(&self.line, t, order, out, scratch)
    }

    fn range(&self) -> ParamRange {
        ParametricCurve::range(&self.line)
    }

    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        let range = ParametricCurve::range(&self.line);
        let (lo, hi) = if self.seed == Seed::Lying { (range.lo, range.hi) } else { (lo, hi) };
        let exact = Bounds::of([self.line.eval_point(lo), self.line.eval_point(hi)]).unwrap();
        let low = exact.low().to_vec3();
        let low = if self.seed == Seed::Short { v(low.x, low.y, low.z.next_up()) } else { low };
        Bounds::of([Point::from_vec3(low), exact.high()]).unwrap()
    }

    fn to_bspline(&self, lo: f64, hi: f64) -> Option<BsplineCurve> {
        let further = |t: f64| self.line.eval_point(t + (hi - lo) * 0.5);
        let poles = vec![further(lo), further(hi)];
        (self.seed == Seed::Lying).then(|| BsplineCurve {
            degree: 1,
            knots: vec![lo, lo, hi, hi],
            poles,
            weights: None,
        })
    }

    fn family(&self) -> FamilyName {
        FamilyName("seeded")
    }

    fn content(&self, out: &mut Content) {
        ParametricCurve::content(&self.line, out);
    }

    fn shape(&self) -> Shape {
        ParametricCurve::shape(&self.line)
    }
}

#[test]
fn a_box_seeded_one_ulp_too_small_changes_a_digest_and_is_caught() {
    let (plane, flat) = (Plane::placed(&Placement::identity()), patch((0.0, 0.0), (4.0, 4.0)));
    // The farthest a box may lie from the plane's and still be scanned: a segment rising from that height, its lowest
    // point exact, is at the box test's edge, where the scan cannot decide its side of the band and says so.
    let edge = (plane.bounds(flat).high().z().millimetres() + tol().band().millimetres()).next_up();
    let line = Line::new(Point::from_vec3(v(1.0, 1.0, edge)), Direction::normalised(v(1.0, 0.0, 1e-3)));
    let line = line.trimmed(0.0, 2.0).unwrap();
    let budget = Budget { steps: 256 };
    let run = |short: bool, culled: bool| {
        let go = if culled { hits } else { scan };
        let seed = if short { Seed::Short } else { Seed::Exact };
        go(&Seeded { line, seed }, span(0.0, 2.0), &plane, flat, tol(), budget)
    };
    let unculled = run(false, false);
    let [report] = unculled.failures.as_slice() else {
        panic!("the pair at the edge is scanned to a report: {unculled:?}")
    };
    assert_eq!((report.code, unculled.tally.undecided), (Code::Undecided, 1), "{report:?}");
    let value = |name: &str| report.value(name).and_then(NamedValue::number).unwrap();
    assert!(
        value("quantity") - value("radius") <= value("band") && value("band") < value("quantity") + value("radius")
    );
    assert_eq!(value("tolerance-in-force"), tol().band().millimetres());
    assert!(report.remedies.is_empty(), "booleans adds move-apart and use-tolerance when it carries the report");
    assert_eq!(digest(&run(false, true)), digest(&unculled), "the right box changes nothing");
    assert_ne!(digest(&run(true, true)), digest(&unculled), "a box one ulp too small drops the pair, and is caught");
}

#[test]
fn a_patch_outside_the_domain_is_refused_whether_or_not_the_boxes_meet() {
    let ball = Sphere::placed(&Placement::identity(), mm(2.0)).unwrap();
    let beyond = patch((0.0, -FRAC_PI_2 - 1e-9), (TAU, FRAC_PI_2));
    for from in [v(5.0, 0.0, 0.0), v(50.0, 50.0, 50.0)] {
        let line = segment(&Placement::identity(), from, v(-1.0, 0.0, 0.0), 3.0);
        let (boxed, unculled) = (
            hits(line.as_ref(), span(0.0, 3.0), &ball, beyond, tol(), BUDGET),
            scan(line.as_ref(), span(0.0, 3.0), &ball, beyond, tol(), BUDGET),
        );
        let [report] = unculled.failures.as_slice() else { panic!("{unculled:?}") };
        assert_eq!(report.code, Code::UndefinedParameter);
        assert_eq!(digest(&boxed), digest(&unculled), "from {from:?}");
    }
}

#[test]
fn a_box_is_the_familys_own_and_never_one_read_from_its_b_spline_form() {
    let (plane, flat) = (Plane::placed(&Placement::identity()), patch((-10.0, -10.0), (10.0, 10.0)));
    let line = Line::new(Point::from_vec3(v(0.5, 0.5, -0.7)), Direction::normalised(v(0.0, 0.0, 1.0)));
    let rising = Seeded { line: line.trimmed(0.0, 2.0).unwrap(), seed: Seed::Lying };
    let crossing = hits(&rising, span(0.0, 2.0), &plane, flat, tol(), BUDGET);
    found(&crossing, (1, 0, 0));
    hit_at(&crossing.crossings[0], 0.7, &plane);
}
