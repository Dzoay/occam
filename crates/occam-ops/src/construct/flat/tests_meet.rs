//! Source: original
//! Where pieces meet, on numbers whose answers are known: a segment and a circle, and two circles, at two points, at
//! the one where they touch, and nowhere; an arc's point square to a slanting segment; an end exactly as far from where
//! two pieces meet as counts as away from it; two pieces that meet at both ends and run along each other; an arc tangent
//! to a segment that turns back along it, and tangent pieces that do not; pieces that meet at one end and cross further
//! on, and the arcs of a needle's tip, which do not; and the limits of `least` and of how finely an arc is cut.

use core::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, PI, SQRT_2};

use occam_math::{atan2, sin_cos};

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
fn pieces_that_run_along_each_other_from_where_they_meet_meet_at_the_middle_of_one() {
    // An arc of radius 1e7 tangent to the segment where it ends turns back along it, s^2 / 2e7 from it at s back from
    // (2, 0): 5e-8 at the segment's middle, inside the band, where they neither cross nor have an end.
    let (band, corner, r) = (1e-7, [[2.0, 0.0]], 1e7);
    let line = seg([0.0, 0.0], [2.0, 0.0]);
    let back = Flat::Arc { c: [2.0, r], r, from: -FRAC_PI_2, sweep: -3e-7 };
    assert_eq!(meet_elsewhere(line, back, &corner, band), Some([1.0, 0.0]), "the first piece's middle");
    let (first, second) = (back.reversed(), line.reversed());
    let met = meet_elsewhere(first, second, &[first.end()], band);
    assert_eq!(met, Some([1.0, 0.0]), "the second piece's middle, run the other way");
    // Tangent where they meet and nowhere else near: the same arc going on, a cusp, and the end of a slot.
    let on = Flat::Arc { c: [2.0, r], r, from: -FRAC_PI_2, sweep: 3e-7 };
    assert_eq!(meet_elsewhere(line, on, &corner, band), None, "going on past the end");
    let cusp = Flat::Arc { c: [2.0, 1.0], r: 1.0, from: -FRAC_PI_2, sweep: -FRAC_PI_2 };
    assert_eq!(meet_elsewhere(line, cusp, &corner, band), None, "turning back away from it");
    let round = Flat::Arc { c: [2.0, 1.0], r: 1.0, from: -FRAC_PI_2, sweep: PI };
    assert_eq!(meet_elsewhere(line, round, &corner, band), None, "round the end of a slot");
}

#[test]
fn pieces_that_meet_at_one_end_and_cross_further_on_meet_where_their_curves_meet_again() {
    // An arc of radius 2.5 about (2.5, 2) from the segment's end at (4, 0), clockwise under it and up across it at
    // (1, 0) to (0, 2): no end or middle of either is near the other, so only the crossing says they meet.
    let band = 1e-7;
    let line = seg([0.0, 0.0], [4.0, 0.0]);
    let from = atan2(-2.0, 1.5);
    let under = Flat::Arc { c: [2.5, 2.0], r: 2.5, from, sweep: -PI - from };
    let met = meet_elsewhere(line, under, &[[4.0, 0.0]], band).unwrap();
    assert!(close(met, [1.0, 0.0]), "a segment, then an arc: {met:?}");
    let (first, second) = (under.reversed(), line.reversed());
    let met = meet_elsewhere(first, second, &[first.end()], band).unwrap();
    assert!(close(met, [1.0, 0.0]), "an arc, then a segment: {met:?}");
    // Arcs of radius 1 about (1, 0) and (0, 1), the first ending and the second beginning at (0, 0): their circles meet
    // again at (1, 1), which both reach, and which the second stops short of if it turns by an eighth.
    let first = Flat::Arc { c: [1.0, 0.0], r: 1.0, from: FRAC_PI_4, sweep: 3.0 * FRAC_PI_4 };
    let second = Flat::Arc { c: [0.0, 1.0], r: 1.0, from: -FRAC_PI_2, sweep: 3.0 * FRAC_PI_4 };
    let met = meet_elsewhere(first, second, &[first.end()], band).unwrap();
    assert!(close(met, [1.0, 1.0]), "two arcs: {met:?}");
    let short = Flat::Arc { c: [0.0, 1.0], r: 1.0, from: -FRAC_PI_2, sweep: FRAC_PI_4 };
    assert_eq!(meet_elsewhere(first, short, &[first.end()], band), None, "one stopping short of where they meet again");
}

#[test]
fn a_segment_folded_back_across_the_one_before_it_crosses_it_where_their_numbers_say() {
    // From a point within the band of the first segment's end, back across it to below it: the two cross at (1.5, 0),
    // strictly within both, which comes before the second's end within the band of the first.
    let (first, back) = (seg([0.0, 0.0], [2.0, 0.0]), seg([2.0, 1e-9], [1.0, -1e-9]));
    let met = meet_elsewhere(first, back, &[[2.0, 0.0]], 1e-7).unwrap();
    assert!(close(met, [1.5, 0.0]), "{met:?}");
}

#[test]
fn the_arcs_of_a_needle_s_tip_meet_only_there() {
    // Arcs of radius 100 and 1 leaving a tip 1e-7 radians apart, each bending away from the other, as construction reads
    // the catalogue's needle's in its plane: they are 1.8e-12 apart 1.8e-6 from the tip, where solving for both places
    // their circles meet at once found them crossing. Their circles meet again 2e-7 behind the tip, beyond both arcs.
    let far = Flat::Arc {
        c: [-5.000_001_812_592_913_5e-6, -100.000_000_000_200_84],
        r: 100.000_000_000_200_97,
        from: 1.570_796_276_794_878_4,
        sweep: -0.004_999_999_999_989_953,
    };
    let near = Flat::Arc {
        c: [-5.000_000_242_500_148_6e-8, 0.999_999_999_999_998_2],
        r: 0.999_999_999_999_999_4,
        from: -1.070_796_276_794_896_2,
        sweep: -0.500_000_000_000_000_2,
    };
    let tip = [-2.223_553_535_201_103_3e-15, 0.0];
    assert!(apart(far.start(), tip) < 1e-14 && apart(near.end(), tip) < 1e-14);
    assert_eq!(meet_elsewhere(far, near, &[tip], 1e-7), None);
    assert_eq!(meet_elsewhere(far, near, &[tip], 1e-9), None, "nor where the band is narrower");
    // The same needle drawn from its tip's own numbers.
    let half = 5e-8;
    let (sin, cos) = sin_cos(half);
    let far = Flat::Arc { c: [-100.0 * sin, -100.0 * cos], r: 100.0, from: FRAC_PI_2 - half, sweep: -0.005 };
    let near = Flat::Arc { c: [-sin, cos], r: 1.0, from: half - FRAC_PI_2 + 0.5, sweep: -0.5 };
    assert!(apart(far.start(), [0.0, 0.0]) < 1e-14 && apart(near.end(), [0.0, 0.0]) < 1e-14);
    assert_eq!(meet_elsewhere(far, near, &[near.end()], 1e-9), None);
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
