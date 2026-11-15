//! Source: original
//! Where pieces meet, on numbers whose answers are known: a segment and a circle, and two circles, at two points, at
//! the one where they touch, and nowhere; an arc's point square to a slanting segment; an end exactly as far from where
//! two pieces meet as counts as away from it; two pieces that meet at both ends and run along each other; and the limits
//! of `least` and of how finely an arc is cut.

use core::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::tests::{circle, seg};
use super::*;

/// Within rounding of each other, for numbers up to a few tens.
fn close(a: P2, b: P2) -> bool {
    apart(a, b) < 1e-12
}

/// Whether `found` is the distinct points `expected`, in any order.
fn same_places(found: &[P2], expected: &[P2]) -> bool {
    found.len() == expected.len() && expected.iter().all(|&e| found.iter().any(|&p| close(p, e)))
}

/// Whether `found` is the point `at` where two curves touch, given once or more.
fn touching_at(found: &[P2], at: P2) -> bool {
    !found.is_empty() && found.iter().all(|&p| close(p, at))
}

#[test]
fn a_segment_meets_a_circle_at_two_points_at_the_one_it_touches_or_nowhere() {
    let (c, r) = ([0.0, 0.0], 5.0);
    let (a, b) = ([2.0, 5.0], [5.0, 2.0]);
    let found = segment_circle(a, b, c, r);
    assert!(same_places(&found, &[[3.0, 4.0], [4.0, 3.0]]), "a chord: {found:?}");
    let on_both = |p: P2| (apart(p, c) - r).abs() < 1e-12 && seg(a, b).distance(p) < 1e-12;
    assert!(found.iter().all(|&p| on_both(p)), "each on the circle and the segment: {found:?}");
    let short = segment_circle([-5.0, 3.0], [0.0, 3.0], c, r);
    assert!(same_places(&short, &[[-4.0, 3.0]]), "a segment that stops inside: {short:?}");
    let grazing = segment_circle([-5.0, 5.0], [5.0, 5.0], c, r);
    assert!(touching_at(&grazing, [0.0, 5.0]), "a tangent: {grazing:?}");
    assert_eq!(segment_circle([-5.0, 6.0], [5.0, 6.0], c, r), Vec::<P2>::new(), "a line clear of it");
}

#[test]
fn two_circles_meet_at_two_points_at_the_one_they_touch_or_nowhere() {
    // Radii 15 and 20, centres 25 apart along (3, 4): a right angle at each point, 9 along and 12 across.
    let (c1, r1, c2, r2) = ([1.0, 2.0], 15.0, [16.0, 22.0], 20.0);
    let found = circles_meet(c1, r1, c2, r2);
    assert!(same_places(&found, &[[-3.2, 16.4], [16.0, 2.0]]), "{found:?}");
    let on_both = |p: P2| (apart(p, c1) - r1).abs() < 1e-12 && (apart(p, c2) - r2).abs() < 1e-12;
    assert!(found.iter().all(|&p| on_both(p)), "each on both circles: {found:?}");
    let with = |c: P2, r: f64| circles_meet([0.0, 0.0], 3.0, c, r);
    assert!(touching_at(&with([3.5, 0.0], 0.5), [3.0, 0.0]), "touching outside: {:?}", with([3.5, 0.0], 0.5));
    assert!(touching_at(&with([2.0, 0.0], 1.0), [3.0, 0.0]), "touching inside: {:?}", with([2.0, 0.0], 1.0));
    assert!(touching_at(&with([-2.0, 0.0], 5.0), [3.0, 0.0]), "inside a larger one: {:?}", with([-2.0, 0.0], 5.0));
    assert_eq!(with([7.0, 0.0], 2.0), Vec::<P2>::new(), "apart");
    assert_eq!(with([1.0, 0.0], 1.0), Vec::<P2>::new(), "one inside the other");
    assert_eq!(with([0.0, 0.0], 1.0), Vec::<P2>::new(), "about one centre");
}

#[test]
fn the_closest_an_arc_comes_to_a_slanting_segment_is_square_to_it() {
    // The centre is 2√2 from the line y = x, square to it at (2, 2), and the circle comes 1 nearer, at 45° below u.
    let (s, round) = (seg([0.0, 0.0], [4.0, 4.0]), circle([0.0, 4.0], 1.0));
    let nearest = [FRAC_1_SQRT_2, 4.0 - FRAC_1_SQRT_2];
    for (d, p) in [closest(s, round), closest(round, s)] {
        assert!((d - (2.0 * SQRT_2 - 1.0)).abs() < 1e-12 && close(p, nearest), "{d} {p:?}");
    }
}

#[test]
fn an_end_folded_back_no_further_than_four_bands_from_where_two_pieces_meet_is_part_of_that_meeting() {
    let band = 0.25;
    let (a, corner) = (seg([0.0, 0.0], [2.0, 0.0]), [[2.0, 0.0]]);
    assert_eq!(meet_elsewhere(a, seg([2.0, 0.0], [1.0, 0.0]), &corner, band), None, "exactly four bands back");
    let further = seg([2.0, 0.0], [0.875, 0.0]);
    assert_eq!(meet_elsewhere(a, further, &corner, band), Some([0.875, 0.0]), "four and a half bands back");
}

#[test]
fn pieces_that_meet_at_both_ends_and_run_along_each_other_meet_at_the_middle_of_the_first() {
    let (band, ends) = (1e-7, [[2.0, 0.0], [0.0, 0.0]]);
    let line = seg([0.0, 0.0], [2.0, 0.0]);
    assert_eq!(meet_elsewhere(line, line.reversed(), &ends, band), Some([1.0, 0.0]), "a segment and the same back");
    let half = Flat::Arc { c: [1.0, 0.0], r: 1.0, from: 0.0, sweep: PI };
    let met = meet_elsewhere(half, half.reversed(), &ends, band).unwrap();
    assert!(close(met, [1.0, 1.0]), "an arc and the same back: {met:?}");
    let other = Flat::Arc { c: [1.0, 0.0], r: 1.0, from: PI, sweep: PI };
    assert_eq!(meet_elsewhere(half, other, &ends, band), None, "the two halves of a circle");
}

#[test]
fn the_least_of_two_equal_numbers_is_the_first_even_where_they_are_noughts_of_either_sign() {
    assert_eq!(least(0.0, -0.0).to_bits(), 0.0_f64.to_bits());
    assert_eq!(least(-0.0, 0.0).to_bits(), (-0.0_f64).to_bits());
    assert_eq!((least(1.0, 2.0), least(2.0, 1.0)), (1.0, 1.0));
}

#[test]
fn an_arc_is_cut_into_as_few_steps_as_its_sag_allows_and_never_more_than_two_to_the_sixteenth() {
    // Steps of √(8 sag / r) = 1/8 radian, eight of which are exactly the sweep.
    let (fits, sag) = (Flat::Arc { c: [0.0, 0.0], r: 8.0, from: 0.0, sweep: 1.0 }, 1.0 / 64.0);
    let points = fits.samples(sag);
    assert_eq!(points.len(), 9, "eight steps exactly as long as the sag allows");
    let middles = points.windows(2).map(|w| scale(add(w[0], w[1]), 0.5));
    assert!(middles.map(|m| fits.distance(m)).all(|d| d <= sag), "and their chords stray no further");
    assert_eq!(circle([0.0, 0.0], 1.0).samples(1e-30).len(), 65_537, "however fine the sag");
}
