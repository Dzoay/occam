//! Source: original
//! The cases the hits' first tests left untried, each found by a mutant that survived them: a dip and a near miss a
//! few bands from a cylinder, short segments within the band and just beyond it, a root exactly at a bisection's
//! midpoint, crossings and touches of surfaces with no analytic form, one so small that
//! the angle's scale shows, and a curve whose points are not finite. Then pinned results: every hit's bits and the
//! decisions made, for scans whose answers do not show how they were found.

use std::sync::Arc;

use occam_geom::{BsplineSurface, Circle, Curve, Cylinder, Line, Plane, SplineCurve, SplineSurface, Surface, UvBox};
use occam_math::Digest;
use occam_report::Code;
use occam_units::{Direction, Placement, Point, Vec3};

use crate::tests_hits::{BUDGET, found, hit_at, mm, patch, placements, saddle, segment, span, tol, v, within};
use crate::{Hits, Span, hits, scan};

/// The band, in millimetres.
fn band() -> f64 {
    tol().band().millimetres()
}

/// A cylinder of radius 2 about the Z axis of each placement, and the line along X at `y`, through `z` = 1, from
/// `x` = -5 to 5: its hits.
fn by_cylinder(y: f64) -> Vec<Hits> {
    let around = patch((0.0, -10.0), (std::f64::consts::TAU, 10.0));
    placements()
        .iter()
        .map(|p| {
            let cylinder = Cylinder::placed(p, mm(2.0)).unwrap();
            let line = segment(p, v(-5.0, y, 1.0), v(1.0, 0.0, 0.0), 10.0);
            hits(line.as_ref(), span(0.0, 10.0), &cylinder, around, tol(), BUDGET)
        })
        .collect()
}

#[test]
fn a_dip_a_few_bands_inside_a_cylinder_is_two_crossings() {
    let y = 2.0 - 3.0 * band();
    let x = (4.0 - y * y).sqrt();
    for (got, p) in by_cylinder(y).iter().zip(placements()) {
        found(got, (2, 0, 0));
        let cylinder = Cylinder::placed(&p, mm(2.0)).unwrap();
        for (hit, t) in got.crossings.iter().zip([5.0 - x, 5.0 + x]) {
            assert!((hit.t - t).abs() < 1e-6, "{} against {t}", hit.t);
            assert!((hit.at.to_vec3() - cylinder.point(hit.uv).to_vec3()).norm() < 1e-9);
        }
    }
}

#[test]
fn a_near_miss_a_few_bands_outside_a_cylinder_is_nothing() {
    for got in by_cylinder(2.0 + 3.0 * band()) {
        found(&got, (0, 0, 0));
    }
}

/// The line along X at `y` from `x` = `-length / 2`, over `start..start + length`, and a cylinder of `radius` about Z
/// through the origin: its hits.
fn tangent(radius: f64, y: f64, length: f64, start: f64) -> Hits {
    let line = Line::new(Point::from_vec3(v(-length * 0.5 - start, y, 0.0)), Direction::normalised(v(1.0, 0.0, 0.0)));
    let line = line.trimmed(start, start + length).unwrap();
    let cylinder = Cylinder::placed(&Placement::identity(), mm(radius)).unwrap();
    hits(&line, span(start, start + length), &cylinder, patch((0.0, -1.0), (std::f64::consts::TAU, 1.0)), tol(), BUDGET)
}

#[test]
fn a_segment_whose_bend_across_its_span_is_half_a_band_lies_in_the_surface_and_one_of_two_bands_touches_it() {
    // A millimetre long, tangent to a cylinder at its middle: its bend is `length² / 8 radius`.
    let (length, start) = (1e-3, 3.0);
    let radius = |bend: f64| length * length / (8.0 * bend * band());
    let within = tangent(radius(0.5), radius(0.5), length, start);
    found(&within, (0, 0, 1));
    assert_eq!((within.overlaps[0].lo, within.overlaps[0].hi), (start, start + length));
    let beyond = tangent(radius(2.0), radius(2.0), length, start);
    found(&beyond, (0, 1, 0));
    assert!((beyond.contacts[0].t - (start + length * 0.5)).abs() < 1e-9, "{beyond:?}");
}

/// A plane through the origin facing Z, and the line rising `rise` across 10 along X, its middle on the plane, over
/// `3..13`: its hits.
fn rising(rise: f64) -> Hits {
    let along = Direction::normalised(v(10.0, 0.0, rise));
    let start = Point::from_vec3(v(-3.0, 0.0, -rise * 0.5) - along.get() * 3.0);
    let length = (100.0 + rise * rise).sqrt();
    let line = Line::new(start, along).trimmed(3.0, 3.0 + length).unwrap();
    let plane = Plane::placed(&Placement::identity());
    hits(&line, span(3.0, 3.0 + length), &plane, patch((-20.0, -20.0), (20.0, 20.0)), tol(), BUDGET)
}

#[test]
fn a_segment_whose_slope_across_its_span_is_half_a_band_lies_in_the_plane_and_one_of_three_bands_crosses_it() {
    let within = rising(band());
    found(&within, (0, 0, 1));
    let beyond = rising(3.0 * band());
    found(&beyond, (1, 0, 0));
    assert!((beyond.crossings[0].t - (3.0 + 5.0)).abs() < 1e-6, "{beyond:?}");
}

#[test]
fn a_root_exactly_at_a_bisections_midpoint_is_found_there_rising_or_falling() {
    let plane = Plane::placed(&Placement::identity());
    for (z, up) in [(-1.0, 1.0), (1.0, -1.0)] {
        let line = Line::new(Point::from_vec3(v(0.25, 0.5, z)), Direction::normalised(v(0.0, 0.0, up)));
        let got = hits(
            &line.trimmed(0.0, 2.0).unwrap(),
            span(0.0, 2.0),
            &plane,
            patch((-1.0, -1.0), (1.0, 1.0)),
            tol(),
            BUDGET,
        );
        found(&got, (1, 0, 0));
        assert_eq!(got.crossings[0].t.to_bits(), 1.0_f64.to_bits(), "from {z}");
    }
}

/// The saddle `z = x y`, and the segment from `from` along `along` over `0..length`: its hits.
fn by_saddle(from: Vec3, along: Vec3, length: f64) -> Hits {
    let line = segment(&Placement::identity(), from, along, length);
    hits(line.as_ref(), span(0.0, length), &saddle(), patch((0.0, 0.0), (1.0, 1.0)), tol(), BUDGET)
}

#[test]
fn a_line_crossing_a_saddle_twice_gives_both_and_one_on_it_tangent_a_contact() {
    let length = 1.8 * std::f64::consts::SQRT_2;
    let twice = by_saddle(v(-0.9, -0.9, 0.1), v(1.0, 1.0, 0.0), length);
    found(&twice, (2, 0, 0));
    for (hit, x) in twice.crossings.iter().zip([-(0.1_f64.sqrt()), 0.1_f64.sqrt()]) {
        hit_at(hit, (x + 0.9) * std::f64::consts::SQRT_2, &saddle());
    }
    for along in [v(1.0, 1.0, 0.0), v(1.0, -1.0, 0.0)] {
        let from = v(-0.9 * along.x, -0.9 * along.y, 0.0);
        let touch = by_saddle(from, along, length);
        found(&touch, (0, 1, 0));
        assert!((touch.contacts[0].t - length * 0.5).abs() < 1e-6, "{touch:?}");
    }
}

#[test]
fn a_crossing_of_a_tiny_surface_at_thirty_degrees_is_a_crossing() {
    let scale = 1e-6;
    let corners = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];
    let poles = corners.map(|(x, y)| Point::from_vec3(v(x, y, x * y) * scale)).to_vec();
    let knots = vec![0.0, 0.0, 1.0, 1.0];
    let form =
        BsplineSurface { degree: (1, 1), count: (2, 2), knots_u: knots.clone(), knots_v: knots, poles, weights: None };
    let tiny = SplineSurface::new(&form).unwrap();
    let (along, length) = (v(3.0_f64.sqrt(), 0.0, 1.0), 2.0 * scale);
    let line =
        segment(&Placement::identity(), v(-0.5, 0.25, 0.0) * scale - along * (0.5 * length * 0.5), along, length);
    let got = scan(line.as_ref(), span(0.0, length), &tiny, patch((0.0, 0.0), (1.0, 1.0)), tol(), BUDGET);
    found(&got, (1, 0, 0));
}

#[test]
fn a_curve_whose_points_are_not_finite_has_its_ends_reported() {
    let lost = Line::new(Point::from_vec3(v(f64::NAN, 0.0, 0.0)), Direction::normalised(v(0.0, 0.0, 1.0)));
    let got =
        scan(&lost.trimmed(0.0, 1.0).unwrap(), span(0.0, 1.0), &saddle(), patch((0.0, 0.0), (1.0, 1.0)), tol(), BUDGET);
    let codes: Vec<Code> = got.failures.iter().map(|r| r.code).collect();
    assert_eq!(codes, [Code::NotFinite, Code::NotFinite], "{got:?}");
}

/// A bicubic B-spline of four spans each way, rolling across `[0, 4]²`, whose box over part of a span is its span's.
fn rolling() -> SplineSurface {
    let heights =
        [0.3, -0.2, 0.5, 0.1, -0.4, 0.2, 0.6, -0.3, 0.1, 0.4, -0.5, 0.2, 0.0, -0.1, 0.3, 0.5, 0.2, -0.3, 0.1, 0.4];
    let grid = (0..7_u8).flat_map(|j| (0..7_u8).map(move |i| (i, j)));
    let poles = grid
        .zip(heights.iter().cycle())
        .map(|((i, j), z)| Point::from_vec3(v(f64::from(i) * 4.0 / 6.0, f64::from(j) * 4.0 / 6.0, *z)))
        .collect();
    let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0, 4.0];
    let form =
        BsplineSurface { degree: (3, 3), count: (7, 7), knots_u: knots.clone(), knots_v: knots, poles, weights: None };
    SplineSurface::new(&form).unwrap()
}

/// A cubic Bezier lying in the plane `z = 0`, and rising out of it.
fn bezier(rise: f64) -> SplineCurve {
    let poles = [v(0.0, 0.0, 0.0), v(1.0, 2.0, 0.0), v(3.0, -1.0, rise), v(4.0, 1.0, 0.0)].map(Point::from_vec3);
    SplineCurve::bezier(poles.to_vec(), None).unwrap()
}

/// A scan as pinned: its crossings, contacts and overlaps found, the decisions it made, the least budget it completes
/// in where the surface is subdivided (along a curve, the decisions show the work), and a digest of every hit's bits.
type Pin = (usize, usize, usize, u64, u32, u64);

/// A cubic Bezier rising from the plane `z = 0` as `t³`, four bands at its end: within the band over its first half
/// and beyond it after.
fn leaving() -> SplineCurve {
    let rise = 4.0 * band();
    let poles =
        [v(0.0, 0.0, 0.0), v(4.0 / 3.0, 0.0, 0.0), v(8.0 / 3.0, 0.0, 0.0), v(4.0, 0.0, rise)].map(Point::from_vec3);
    SplineCurve::bezier(poles.to_vec(), None).unwrap()
}

/// Everything a scan found, every number by its bits, and the decisions it made.
fn pinned(got: &Hits) -> Pin {
    let mut out = Digest::EMPTY;
    for hit in got.crossings.iter().chain(&got.contacts) {
        for x in [hit.t, hit.uv.u(), hit.uv.v()] {
            out.write_value(x);
        }
    }
    for overlap in &got.overlaps {
        out.write_value(overlap.lo);
        out.write_value(overlap.hi);
    }
    out.write_bytes(format!("{:?}", got.failures.iter().map(|r| r.code).collect::<Vec<_>>()).as_bytes());
    (got.crossings.len(), got.contacts.len(), got.overlaps.len(), got.tally.decided, 0, out.finish())
}

/// The most a pinned scan is given in [`least`]: five times the costliest's, so that a change that multiplies the work
/// shows quickly, as a scan that does not complete.
const MOST: u32 = 10_000;

/// The least budget `case`'s scan completes in, up to [`MOST`]: what it costs, which a change to how it works shows
/// though what it finds does not.
fn least(case: &Case) -> u32 {
    let (_, curve, span, surface, patch) = case;
    let completes = |steps: u32| {
        let got = scan(curve.as_ref(), *span, surface.as_ref(), *patch, tol(), crate::Budget { steps });
        got.failures.iter().all(|r| r.code != Code::HitsIncomplete)
    };
    if !completes(MOST) {
        return MOST.saturating_add(1);
    }
    let (mut lo, mut hi) = (1, MOST);
    while lo < hi {
        let mid = lo.midpoint(hi);
        if completes(mid) { hi = mid } else { lo = mid.saturating_add(1) }
    }
    lo
}

/// The cases pinned: a name, a curve over a span, and a surface over a patch.
pub(crate) type Case = (&'static str, Arc<dyn Curve>, Span, Arc<dyn Surface>, UvBox);

/// A line through, touching, dipping into and missing a cylinder of radius 2, under each placement.
fn by_cylinders() -> Vec<Case> {
    let mut all: Vec<Case> = Vec::new();
    let side = patch((0.0, -10.0), (std::f64::consts::TAU, 10.0));
    for (i, p) in placements().iter().enumerate() {
        let cylinder: Arc<dyn Surface> = Arc::new(Cylinder::placed(p, mm(2.0)).unwrap());
        for (name, y) in [("through", 0.5), ("touch", 2.0), ("dip", 2.0 - 3.0 * band()), ("miss", 2.0 + 3.0 * band())] {
            let name: &'static str = Box::leak(format!("{name} {i}").into_boxed_str());
            let line = segment(p, v(-5.0, y, 1.0), v(1.0, 0.0, 0.0), 10.0);
            all.push((name, line, span(0.0, 10.0), Arc::clone(&cylinder), side));
        }
    }
    all
}

pub(crate) fn cases() -> Vec<Case> {
    let mut all = by_cylinders();
    let plane: Arc<dyn Surface> = Arc::new(Plane::placed(&Placement::identity()));
    let flat = patch((-10.0, -10.0), (10.0, 10.0));
    for (name, rise) in [("bezier lying", 0.0), ("bezier rising", 0.5)] {
        all.push((name, Arc::new(bezier(rise)), span(0.0, 1.0), Arc::clone(&plane), flat));
    }
    let tiny = Line::new(Point::from_vec3(v(0.25, 0.5, -1.0)), Direction::normalised(v(0.0, 0.0, 1.0)));
    let ulps = 4.0 * f64::EPSILON;
    all.push((
        "ulps",
        Arc::new(tiny.trimmed(1.0 - ulps, 1.0 + ulps).unwrap()),
        span(1.0 - ulps, 1.0 + ulps),
        Arc::clone(&plane),
        flat,
    ));
    all.extend(subdivided());
    all.extend(edges());
    all
}

/// Lines and a Bezier against a saddle and a rolling bicubic, surfaces with no analytic form.
fn subdivided() -> Vec<Case> {
    let mut all: Vec<Case> = Vec::new();
    let saddle: Arc<dyn Surface> = Arc::new(saddle());
    let rolling: Arc<dyn Surface> = Arc::new(rolling());
    let length = 1.8 * std::f64::consts::SQRT_2;
    for (name, from, along) in [
        ("saddle twice", v(-0.9, -0.9, 0.1), v(1.0, 1.0, 0.0)),
        ("saddle touch", v(-0.9, -0.9, 0.0), v(1.0, 1.0, 0.0)),
        ("saddle down", v(0.5, 0.25, 2.0), v(0.0, 0.0, -1.0)),
    ] {
        all.push((
            name,
            segment(&Placement::identity(), from, along, length),
            span(0.0, length),
            Arc::clone(&saddle),
            patch((0.0, 0.0), (1.0, 1.0)),
        ));
    }
    for (name, from, along) in [
        ("rolling across", v(0.1, 0.3, 0.2), v(1.0, 0.9, -0.05)),
        ("rolling down", v(2.3, 1.7, 2.0), v(0.1, -0.05, -1.0)),
    ] {
        all.push((
            name,
            segment(&Placement::identity(), from, along, 4.0),
            span(0.0, 4.0),
            Arc::clone(&rolling),
            patch((0.0, 0.0), (4.0, 4.0)),
        ));
    }
    all.push(("bezier on rolling", Arc::new(bezier(0.5)), span(0.0, 1.0), rolling, patch((0.0, 0.0), (4.0, 4.0))));
    all
}

/// The cases at the edges of a scan's arithmetic: a circle touching a cylinder from inside exactly at the middle of
/// its span, where the distance's slope is exactly zero, and a segment ending on the saddle.
fn edges() -> Vec<Case> {
    let inside = within(&Placement::identity(), v(0.0, 0.0, 1.0), 0.0, v(1.0, 0.0, 0.0));
    let circle: Arc<dyn Curve> = Arc::new(Circle::placed(&inside, mm(1.0)).unwrap());
    let cylinder: Arc<dyn Surface> = Arc::new(Cylinder::placed(&Placement::identity(), mm(2.0)).unwrap());
    let short = segment(&Placement::identity(), v(0.5, 0.25, 2.0), v(0.0, 0.0, -1.0), 2.0 - 0.125);
    let starting = segment(&Placement::identity(), v(0.5, 0.25, 2.0), v(0.0, 0.0, -1.0), 4.0);
    let long = segment(&Placement::identity(), v(-10.0, 0.3, 0.05), v(1.0, 0.0, 0.01), 20.0);
    let through = segment(&Placement::identity(), v(-10.0, 0.0, -0.1), v(1.0, 0.0, 0.01), 20.0);
    let saddle: Arc<dyn Surface> = Arc::new(saddle());
    vec![
        ("circle inside", circle, span(-1.0, 1.0), cylinder, patch((0.0, -1.0), (std::f64::consts::TAU, 1.0))),
        ("saddle starting", starting, span(2.0 - 0.125, 3.0), Arc::clone(&saddle), patch((0.0, 0.0), (1.0, 1.0))),
        (
            "bezier leaving",
            Arc::new(leaving()),
            span(0.0, 1.0),
            Arc::new(Plane::placed(&Placement::identity())),
            patch((-10.0, -10.0), (10.0, 10.0)),
        ),
        ("saddle long", long, span(0.0, 20.0), Arc::clone(&saddle), patch((0.0, 0.0), (1.0, 1.0))),
        ("saddle isoline", through, span(0.0, 20.0), Arc::clone(&saddle), patch((0.0, 0.5), (1.0, 0.5))),
        ("saddle ending", short, span(0.0, 2.0 - 0.125), saddle, patch((0.0, 0.0), (1.0, 1.0))),
    ]
}

/// What each case gave when pinned: crossings, contacts and overlaps found, the decisions made, and the digest of
/// every hit's bits. A change to how a scan works, not only to what it finds, shows here. The digests are what a scan
/// gave, so they detect a change and prove nothing: where the hits must be, from the geometry, is asserted by the tests
/// above and [`crate::tests_hits`]' of the same constructions, and for the rest in [`crate::tests_placed`].
const PINNED: [(&str, Pin); 27] = [
    ("through 0", (2, 0, 0, 107, 0, 0xD36B_710C_C1B2_8263)),
    ("touch 0", (0, 1, 0, 107, 0, 0xFF92_69B9_72F2_EBE3)),
    ("dip 0", (2, 0, 0, 168, 0, 0xC60E_9832_146A_A12A)),
    ("miss 0", (0, 0, 0, 51, 0, 0x0961_2B07_B5EC_B5A5)),
    ("through 1", (2, 0, 0, 107, 0, 0xD5AA_B528_F5F8_6D23)),
    ("touch 1", (0, 1, 0, 11_383, 0, 0x2633_0446_C447_3E71)),
    ("dip 1", (2, 0, 0, 12_731, 0, 0x7C43_241E_49FB_B9FA)),
    ("miss 1", (0, 0, 0, 13_540, 0, 0x0961_2B07_B5EC_B5A5)),
    ("through 2", (2, 0, 0, 107, 0, 0x6AB9_E88F_1820_DC04)),
    ("touch 2", (0, 1, 0, 7848, 0, 0x6438_9BA5_A4E9_E52A)),
    ("dip 2", (2, 0, 0, 10_996, 0, 0x3645_B1EA_60E2_E0F7)),
    ("miss 2", (0, 0, 0, 9987, 0, 0x0961_2B07_B5EC_B5A5)),
    ("bezier lying", (0, 0, 1, 1, 0, 0xE353_E7F5_9B38_1A8C)),
    ("bezier rising", (0, 2, 0, 108, 0, 0x8468_3053_6E2B_F995)),
    ("ulps", (0, 0, 1, 1, 0, 0x94F3_6CC9_D6D6_4E53)),
    ("saddle twice", (2, 0, 0, 8, 259, 0xBF1C_3C98_6516_7215)),
    ("saddle touch", (0, 1, 0, 29, 419, 0x1058_395F_68BA_5421)),
    ("saddle down", (1, 0, 0, 13, 157, 0xFA94_9497_143C_0BFE)),
    ("rolling across", (3, 0, 0, 12, 363, 0x97C8_918E_0EDB_F883)),
    ("rolling down", (1, 0, 0, 4, 71, 0x15DF_FA1B_2F3D_63BB)),
    ("bezier on rolling", (2, 0, 0, 9, 477, 0xF702_2484_2D14_A40A)),
    ("circle inside", (0, 1, 0, 284, 0, 0x5A41_524C_5095_938D)),
    ("saddle starting", (0, 1, 0, 2, 105, 0xFA94_9497_143C_0BFE)),
    ("bezier leaving", (0, 1, 0, 84, 0, 0x1927_801C_A3BA_5085)),
    ("saddle long", (1, 0, 0, 4, 79, 0x60E4_186B_5F61_DF14)),
    ("saddle isoline", (1, 0, 0, 5, 113, 0xC50B_A075_9831_D989)),
    ("saddle ending", (0, 1, 0, 2, 109, 0xFA94_9497_143C_0BFE)),
];

#[test]
fn every_pinned_scan_finds_the_same_bits_with_the_same_decisions() {
    let got: Vec<(&str, _)> = cases()
        .into_iter()
        .map(|case| {
            let (name, curve, span, surface, patch) = &case;
            let (crossings, contacts, overlaps, decided, _, bits) =
                pinned(&scan(curve.as_ref(), *span, surface.as_ref(), *patch, tol(), BUDGET));
            let steps = if surface.analytic(tol()).is_none() { least(&case) } else { 0 };
            (*name, (crossings, contacts, overlaps, decided, steps, bits))
        })
        .collect();
    for ((name, got), want) in got.iter().zip(PINNED) {
        assert_eq!((*name, *got), want);
    }
    assert_eq!(got.len(), PINNED.len());
}

#[test]
fn a_span_halved_to_the_doubles_resolution_is_halved_no_further() {
    // So far from the origin that no point can be placed against the band, and so short that halving it reaches
    // neighbouring doubles in six steps: 127 pieces in all, and two ends that cannot be decided.
    let far = Line::new(Point::from_vec3(v(1e9, 0.0, -1.0)), Direction::normalised(v(0.0, 0.0, 1.0)));
    let (lo, hi) = (1.0 - 32.0 * f64::EPSILON, 1.0 + 32.0 * f64::EPSILON);
    let plane = Plane::placed(&Placement::identity());
    let flat = patch((-2e9, -2e9), (2e9, 2e9));
    let got = scan(&far.trimmed(lo, hi).unwrap(), span(lo, hi), &plane, flat, tol(), crate::Budget { steps: 200 });
    let codes: Vec<Code> = got.failures.iter().map(|r| r.code).collect();
    assert_eq!(codes, [Code::Undecided, Code::Undecided], "{got:?}");
}

#[test]
fn a_crossing_of_a_patch_whose_box_barely_shrinks_is_found() {
    // A patch of the saddle with no width in v: its box keeps the whole knot span's height in v, so halving it in u
    // shrinks the box less and less, and never to a leaf's size.
    let through = segment(&Placement::identity(), v(-10.0, 0.0, -0.1), v(1.0, 0.0, 0.01), 20.0);
    let got = scan(through.as_ref(), span(0.0, 20.0), &saddle(), patch((0.0, 0.5), (1.0, 0.5)), tol(), BUDGET);
    found(&got, (1, 0, 0));
    hit_at(&got.crossings[0], 10.0 * (1.0 + 1e-4_f64).sqrt(), &saddle());
}
