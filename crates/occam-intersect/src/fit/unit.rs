//! Source: original
//! The fit's own measures, each against an answer known by construction: a parameter carried round, the floor of a
//! fit, the Gauss-Newton step to a foot, a distance along a normal, the largest gap noted, and a station settled
//! between two.

use std::f64::consts::{PI, TAU};

use occam_geom::{ParamRange, SplineSurface, Surface, TrimmedSurface, UvBox, Wrap};
use occam_math::Interval;
use occam_units::{ParamKind, UvPoint, Vec3};

use super::{ROUNDING, Worst, between, floored, gaps, round_to, step};
use crate::correct::Pair;
use crate::trace::Station;
use crate::trace_tests::{BAND, level, point, tol, upright};

fn range(lo: f64, hi: f64, wrap: Wrap) -> ParamRange {
    ParamRange { lo, hi, wrap, kind: ParamKind::Angle }
}

#[test]
fn a_parameter_is_carried_round_only_where_it_comes_round() {
    let turn = range(-PI, PI, Wrap::Periodic);
    assert!((round_to(6.0, 0.0, turn) - (6.0 - TAU)).abs() < 1e-15, "a turn back");
    assert!((round_to(-6.5, 0.0, turn) - (TAU - 6.5)).abs() < 1e-15, "a turn on");
    assert_eq!(round_to(0.5, 0.4, turn), 0.5, "already within half a turn");
    let others = [
        range(0.0, TAU, Wrap::Open),
        range(0.0, TAU, Wrap::Closed),
        range(0.0, f64::INFINITY, Wrap::Periodic),
        range(2.0, 2.0, Wrap::Periodic),
    ];
    for other in others {
        assert_eq!(round_to(6.0, 0.1, other).to_bits(), 6.0_f64.to_bits(), "{other:?}");
    }
}

#[test]
fn no_fit_is_below_the_rounding_of_its_stations_size() {
    let points = [point(1.0, -5.0, 2.0), point(0.5, 3.0, -4.0)];
    let floor = ROUNDING * 6.0;
    assert_eq!(floored(0.0, &points), floor);
    assert_eq!(floored(floor * 0.5, &points), floor);
    assert_eq!(floored(1e-9, &points), 1e-9);
    assert!(floored(f64::NAN, &points).is_nan());
    assert_eq!(floored(0.0, &[]), ROUNDING);
}

/// The plane `u a + v b`, `a = (2, 0, 0)` and `b = (1, 3, 0)`, as a bilinear patch, or with its edge `v = 1` collapsed
/// to a point above it, where its derivatives span no plane.
fn sheet(collapsed: bool) -> SplineSurface {
    let apex = point(0.5, 3.0, 1.0);
    let far = if collapsed { [apex, apex] } else { [point(1.0, 3.0, 0.0), point(3.0, 3.0, 0.0)] };
    SplineSurface::bezier((2, 2), vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0), far[0], far[1]], None).unwrap()
}

#[test]
fn the_step_to_a_foot_is_how_far_its_parameters_move() {
    // Off the sheet along its normal by 0.7, and along it by parameters 0.03 and -0.02, which its skewed derivatives
    // turn into a point that only the full normal equations bring back.
    let (s, uv) = (sheet(false), UvPoint::new(0.25, 0.5));
    let p = s.point(UvPoint::new(0.28, 0.48)).to_vec3() + Vec3::new(0.0, 0.0, 0.7);
    let got = step(&s, uv, p).unwrap();
    assert!((got - (0.03_f64 * 0.03 + 0.02 * 0.02).sqrt()).abs() < 1e-14, "{got}");
    assert_eq!(step(&sheet(true), UvPoint::new(0.4, 1.0), p), None, "where the derivatives span no plane");
}

#[test]
fn a_distance_from_a_surface_is_along_its_normal_or_where_it_has_none_all_the_way() {
    let (s, uv) = (sheet(false), UvPoint::new(0.25, 0.5));
    let at = s.point(uv).to_vec3() + Vec3::new(0.3, 0.0, 0.4);
    let (off, whole) = gaps(&s, uv, at);
    assert!((off.mid().abs() - 0.4).abs() < 1e-15 && (whole.mid() - 0.5).abs() < 1e-15, "{off:?} {whole:?}");
    let (off, whole) = gaps(&sheet(true), UvPoint::new(0.4, 1.0), at);
    assert!(whole.mid().is_finite() && off == whole, "no normal at the collapsed edge: {off:?} {whole:?}");
}

#[test]
fn the_largest_gap_noted_is_the_far_end_of_its_enclosure_and_none_beats_a_nan() {
    let (first, second) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0));
    let mut worst = Worst::NONE;
    worst.note(Interval::new(-2.0, 0.5), first);
    worst.note(Interval::new(1.0, 0.25), second);
    assert!((worst.gap - 2.5).abs() < 1e-15 && worst.at == first, "{}", worst.gap);
    worst.note(Interval::new(3.0, 0.0), second);
    assert!((worst.gap - 3.0).abs() < 1e-15 && worst.at == second);
    worst.note(Interval::new(f64::NAN, 0.0), first);
    worst.note(Interval::new(9.0, 0.0), second);
    assert!(worst.gap.is_nan() && worst.at == first, "a gap that is not a number stays the largest");
}

/// Two stations on the Y axis, where the planes `z = 0` and `x = 0` meet, `apart` apart, each with the parameters
/// on both planes of the point `skew` further along than itself: the second's are wrong by `skew`.
fn on_the_axis(apart: f64, skew: f64) -> [Station; 2] {
    let at = |y: f64, ahead: f64| Station {
        at: point(0.0, y, 0.0),
        on_a: UvPoint::new(0.0, y + ahead),
        on_b: UvPoint::new(0.0, y + ahead),
    };
    [at(0.0, 0.0), at(apart, skew)]
}

#[test]
fn a_new_station_is_settled_halfway_along_the_chord_whatever_the_seed() {
    let (flat, wall) = (level(0.0), upright(0.0));
    assert_eq!(wall.point(UvPoint::new(0.0, 0.7)), point(0.0, 0.7, 0.0), "the wall's parameters along the axis");
    let pair = Pair::new(&flat, &wall, tol());
    for (apart, skew) in [(0.5, 0.0), (1e-3, 0.04)] {
        let [before, after] = on_the_axis(apart, skew);
        let settled = between(&pair, &before, &after).unwrap_or_else(|| panic!("{apart}: not settled"));
        let v = settled.at.to_vec3();
        assert!((v.y - apart / 2.0).abs() <= BAND && v.x.abs() <= BAND && v.z.abs() <= BAND, "{apart}: {settled:?}");
    }
}

#[test]
fn a_station_settled_outside_a_surfaces_domain_is_not_taken() {
    let window = |v: f64| UvBox::new(UvPoint::new(-1.0, v), UvPoint::new(1.0, 2.0));
    let wall = upright(0.0);
    let [before, after] = on_the_axis(0.5, 0.0);
    for (from, taken) in [(0.4, false), (-1.0, true)] {
        let patch = TrimmedSurface::new(level(0.0), window(from)).unwrap();
        let settled = between(&Pair::new(&patch, &wall, tol()), &before, &after);
        assert_eq!(settled.is_some(), taken, "the patch's v from {from}: {settled:?}");
    }
}
