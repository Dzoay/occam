//! Source: original
//! Where each pinned scan's hits are, from its geometry and not from a scan. [`crate::tests_pinned`] pins every hit's
//! bits, which only a scan can give, so a wrong answer once pinned would stay. Here every hit of every pinned case is
//! a point of its curve and of its surface, inside the span and the patch; the cases with a closed form that no other
//! test places are at its values; and those on the rolling surface, which has none, lie between two samples of the
//! curve on either side of it.

use occam_geom::{Curve, Surface};
use occam_units::UvPoint;

use crate::tests_hits::{BUDGET, apart, found, tol};
use crate::tests_pinned::{Case, cases};
use crate::{Hits, scan};

/// How far a hit may be from where its geometry puts it, in millimetres and in the curve's parameter.
const NEAR: f64 = 1e-9;

fn scanned(case: &Case) -> Hits {
    let (_, curve, span, surface, patch) = case;
    scan(curve.as_ref(), *span, surface.as_ref(), *patch, tol(), BUDGET)
}

/// The pinned case of that name.
fn case(name: &str) -> Case {
    cases().into_iter().find(|case| case.0 == name).unwrap()
}

#[test]
fn every_pinned_hit_is_a_point_of_its_curve_and_its_surface_inside_the_span_and_the_patch() {
    let mut points = 0_usize;
    for case in cases() {
        let got = scanned(&case);
        let (name, curve, span, surface, patch) = &case;
        for hit in got.crossings.iter().chain(&got.contacts) {
            assert!(span.lo <= hit.t && hit.t <= span.hi, "{name}: {hit:?} outside {span:?}");
            let (u, v) = (hit.uv.u(), hit.uv.v());
            assert!(
                patch.lo.u() <= u && u <= patch.hi.u() && patch.lo.v() <= v && v <= patch.hi.v(),
                "{name}: {hit:?}"
            );
            assert!(apart(hit.at, curve.point(hit.t)) < NEAR, "{name}: {hit:?} is off its curve");
            assert!(apart(hit.at, surface.point(hit.uv)) < NEAR, "{name}: {hit:?} is off its surface");
            points += 1;
        }
        for overlap in &got.overlaps {
            assert!(span.lo <= overlap.lo && overlap.lo < overlap.hi && overlap.hi <= span.hi, "{name}: {overlap:?}");
        }
    }
    assert_eq!(points, 33, "23 crossings and 10 contacts in the 27 cases, so the loop compares something");
}

/// What a case's geometry gives: the parameters of its crossings and of its contacts, and its overlaps.
type Placed = (&'static str, Vec<f64>, Vec<f64>, Vec<(f64, f64)>);

/// The pinned cases with a closed form that no other test places.
fn closed_forms() -> Vec<Placed> {
    let ulps = 4.0 * f64::EPSILON;
    vec![
        // The Bezier's height is `3 rise t² (1 - t)`: with no rise it lies in the plane, and with one it leaves the
        // plane tangent to it at its start and comes back to it at its end.
        ("bezier lying", vec![], vec![], vec![(0.0, 1.0)]),
        ("bezier rising", vec![], vec![0.0, 1.0], vec![]),
        // A line through the plane at 1, cut four ulps either side of it: all of it is within the band.
        ("ulps", vec![], vec![], vec![(1.0 - ulps, 1.0 + ulps)]),
        // Down from a height of 2 over (0.5, 0.25), where the saddle `z = x y` is at 0.125: through it, or a span that
        // starts or ends there.
        ("saddle down", vec![1.875], vec![], vec![]),
        ("saddle starting", vec![], vec![1.875], vec![]),
        ("saddle ending", vec![], vec![1.875], vec![]),
        // A circle of radius 1 about (1, 0, 0) inside a cylinder of radius 2 about Z reaches it at (2, 0, 0) alone.
        ("circle inside", vec![], vec![0.0], vec![]),
        // A height of four bands times `t³`: the span starts on the plane and does not come back to it.
        ("bezier leaving", vec![], vec![0.0], vec![]),
        // At `y` = 0.3 the line's height `0.15 + 0.01 x` meets the saddle's `0.3 x` at `x` = 0.15 / 0.29, and it
        // starts at `x` = -10 with a length of `sqrt(1 + 0.01²)` for each unit of `x`.
        ("saddle long", vec![(10.0 + 0.15 / 0.29) * (1.0_f64 + 1e-4).sqrt()], vec![], vec![]),
    ]
}

#[test]
fn a_pinned_scan_with_a_closed_form_finds_its_values() {
    for (name, crossings, contacts, overlaps) in closed_forms() {
        let got = scanned(&case(name));
        found(&got, (crossings.len(), contacts.len(), overlaps.len()));
        for (hit, t) in got.crossings.iter().zip(crossings).chain(got.contacts.iter().zip(contacts)) {
            assert!((hit.t - t).abs() < NEAR, "{name}: {} against {t}", hit.t);
        }
        let spans: Vec<(f64, f64)> = got.overlaps.iter().map(|overlap| (overlap.lo, overlap.hi)).collect();
        assert_eq!(spans, overlaps, "{name}");
    }
}

/// How far the curve is above the rolling surface at `t`. The surface's poles are a grid, so its `x` depends on `u`
/// alone and its `y` on `v` alone, each rising across `0..4`: the point of it under the curve's is found by halving.
fn above(curve: &dyn Curve, surface: &dyn Surface, t: f64) -> f64 {
    let point = curve.point(t).to_vec3();
    let solve = |want: f64, at: &dyn Fn(f64) -> f64| {
        let (mut lo, mut hi) = (0.0_f64, 4.0_f64);
        for _ in 0..60 {
            let mid = lo.midpoint(hi);
            if at(mid) < want { lo = mid } else { hi = mid }
        }
        lo.midpoint(hi)
    };
    let u = solve(point.x, &|u| surface.point(UvPoint::new(u, 0.0)).to_vec3().x);
    let v = solve(point.y, &|v| surface.point(UvPoint::new(0.0, v)).to_vec3().y);
    let under = surface.point(UvPoint::new(u, v)).to_vec3();
    assert!((under.x - point.x).abs() < NEAR && (under.y - point.y).abs() < NEAR, "{under:?} is under {point:?}");
    point.z - under.z
}

#[test]
fn a_pinned_scan_of_the_rolling_surface_crosses_it_where_the_sampled_curve_changes_sides() {
    // 2000 steps along each curve: a crossing is between two samples on opposite sides of the surface, and there
    // are as many crossings as such pairs, where no two crossings are within a step.
    let mut all = Vec::new();
    for name in ["rolling across", "rolling down", "bezier on rolling"] {
        let case = case(name);
        let (_, curve, span, surface, _) = &case;
        let samples: Vec<(f64, f64)> = (0..=2000_u16)
            .map(|i| span.lo + (span.hi - span.lo) * f64::from(i) / 2000.0)
            .map(|t| (t, above(curve.as_ref(), surface.as_ref(), t)))
            .collect();
        let between: Vec<(f64, f64)> =
            samples.windows(2).filter(|pair| pair[0].1 * pair[1].1 < 0.0).map(|pair| (pair[0].0, pair[1].0)).collect();
        let got = scanned(&case);
        found(&got, (between.len(), 0, 0));
        for (hit, (lo, hi)) in got.crossings.iter().zip(&between) {
            assert!(*lo < hit.t && hit.t < *hi, "{name}: {} is not between {lo} and {hi}", hit.t);
            assert!(above(curve.as_ref(), surface.as_ref(), hit.t).abs() < NEAR, "{name}: {hit:?}");
        }
        all.push(between.len());
    }
    assert_eq!(all, [3, 1, 2], "counted from the samples, not from the scan");
}
