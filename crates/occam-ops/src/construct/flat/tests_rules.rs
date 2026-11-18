//! Source: original
//! This file's rules for two pieces in a plane, beside occam-intersect's for the same numbers, so that where they
//! differ is pinned whichever is used (the review of 2026-11-17, item 12). occam-intersect has no curve against a
//! curve: the nearest it offers is a curve against a surface, [`hits`], as a boolean asks of an edge against a face. So
//! here the first piece is a curve in the plane `z = 0`, and the second the surface that stands on it: a segment is a
//! wall, the upright plane through it with a patch as long as it is, and a circle a cylinder about it.
//!
//! The two agree that a tangent line and circle meet, and that pieces well clear of each other do not. They differ in
//! four rules, one test each:
//!
//! - what a meeting is: here one kind, a touch, with one place; there a crossing, a contact or an overlap;
//! - a chord shallower than the band: here two crossings; there one contact where the line comes nearest, the bounded
//!   touch of [the decision](../../../../../docs/decisions/2026-11-16-every-smooth-tangency-is-a-bounded-touch.md);
//! - past an end: here a piece within the band of the other's end touches it; there a patch has exact edges, and a hit
//!   beyond one is no hit, however near;
//! - at the band's edge: here a distance of exactly the band is a touch, decided on the plain number; there a value
//!   whose enclosure reaches across the band's edge is `decide.undecided` (DETERMINISM.md rule 6).

use core::f64::consts::{FRAC_PI_2, TAU};

use occam_geom::{Curve, Cylinder, Line, Plane, Surface, UvBox};
use occam_intersect::{Budget, Hits, Overlap, Span, hits};
use occam_report::Code;
use occam_units::{Angle, Direction, Length, Local, Placement, Point, Tolerance, UvPoint, Vec3, World};

use super::tests::{circle, seg};
use super::*;
use crate::construct::touch::first_touch;

const BAND: f64 = 1e-7;

/// The segment from the origin to `(4, 0)`, which every test sets a second piece against.
const BASE: Flat = Flat::Seg { a: [0.0, 0.0], b: [4.0, 0.0] };

/// Where `x` and `y`, each a run of its own, touch within the band: what a face's loops are refused for.
fn touch(x: Flat, y: Flat) -> Option<P2> {
    first_touch(&[(&[x], false), (&[y], false)], BAND).map(|t| t.at)
}

/// The wall on [`BASE`] made `longer`: the plane `y = 0`, `u` along X and `v` along Z, over `u` from 0 to 4 and
/// `longer`.
fn wall(longer: f64) -> (Plane, UvBox) {
    let upright = Placement::rotation(Direction::normalised(Vec3::new(1.0, 0.0, 0.0)), Angle::from_radians(FRAC_PI_2));
    let placed: Placement<Local, World> = Placement::identity().then(&upright);
    (Plane::placed(&placed), UvBox::new(UvPoint::new(0.0, -1.0), UvPoint::new(4.0 + longer, 1.0)))
}

/// What occam-intersect finds of the segment from `a` to `b`, in the plane `z = 0`, on `surface` over `patch`: how many
/// crossings, contacts and overlaps, and the codes of its failures; and all it found.
fn met(a: P2, b: P2, (surface, patch): (&dyn Surface, UvBox)) -> ((usize, usize, usize), Vec<Code>, Hits) {
    let tol = Tolerance::new(Length::from_millimetres(BAND), Angle::from_radians(1e-9)).unwrap();
    let along = Direction::normalised(Vec3::new(b[0] - a[0], b[1] - a[1], 0.0));
    let line = Line::new(Point::from_vec3(Vec3::new(a[0], a[1], 0.0)), along);
    let span = Span { lo: 0.0, hi: apart(a, b) };
    let found = hits(&line as &dyn Curve, span, surface, patch, tol, Budget { steps: 100_000 });
    let counts = (found.crossings.len(), found.contacts.len(), found.overlaps.len());
    (counts, found.failures.iter().map(|r| r.code).collect(), found)
}

/// [`met`] for the segment along X from `x = -5` to `5` at the height `y`, on the cylinder of radius 2 about Z.
fn on_cylinder(y: f64) -> ((usize, usize, usize), Vec<Code>, Hits) {
    let cylinder = Cylinder::placed(&Placement::identity(), Length::from_millimetres(2.0)).unwrap();
    let patch = UvBox::new(UvPoint::new(0.0, -1.0), UvPoint::new(TAU, 1.0));
    met([-5.0, y], [5.0, y], (&cylinder, patch))
}

#[test]
fn a_tangent_line_and_circle_meet_under_both_rules_here_as_a_touch_and_there_as_a_contact() {
    let round = circle([0.0, 0.0], 2.0);
    for (y, what) in [(2.0, "tangent"), (2.0 + 0.5 * BAND, "half the band clear of it")] {
        let at = touch(seg([-5.0, y], [5.0, y]), round).unwrap();
        assert!(apart(at, [0.0, 2.0]) <= BAND, "{what}: a touch where the circle is nearest the line: {at:?}");
        let (counts, failures, found) = on_cylinder(y);
        assert_eq!((counts, failures), ((0, 1, 0), vec![]), "{what}: one contact, not a crossing");
        assert!((found.contacts[0].t - 5.0).abs() < 1e-9, "{what}: {:?}", found.contacts);
    }
    assert_eq!(touch(seg([-5.0, 2.0 + 2.0 * BAND], [5.0, 2.0 + 2.0 * BAND]), round), None, "twice the band clear");
    assert_eq!(on_cylinder(2.0 + 2.0 * BAND).0, (0, 0, 0), "and there");
}

#[test]
fn a_chord_shallower_than_the_band_is_two_crossings_here_and_one_contact_there() {
    // The line passes half the band inside the circle of radius 2, and cuts it at x = -half and +half, where half^2
    // is 4 - y^2: about 4.5e-4 either side of where it is nearest, far more than the band.
    let y = 2.0 - 0.5 * BAND;
    let half = (4.0 - y * y).sqrt();
    assert!(half > 1000.0 * BAND, "{half}");
    let (line, round) = (seg([-5.0, y], [5.0, y]), circle([0.0, 0.0], 2.0));
    let found = crossings(line, round);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(apart(found[0], [-half, y]) < 1e-9 && apart(found[1], [half, y]) < 1e-9, "{found:?}");
    assert!(apart(touch(line, round).unwrap(), [-half, y]) < 1e-9, "the touch is the first of them");
    let (counts, failures, there) = on_cylinder(y);
    assert_eq!((counts, failures), ((0, 1, 0), vec![]), "one contact, where the line is nearest: a bounded touch");
    assert!((there.contacts[0].t - 5.0).abs() < 1e-9, "{:?}", there.contacts);
    let deep = on_cylinder(2.0 - 10.0 * BAND);
    assert_eq!((deep.0, deep.1), ((2, 0, 0), vec![]), "a chord deeper than the band is two crossings there too");
}

#[test]
fn an_end_resting_on_the_other_is_a_touch_here_and_a_contact_there() {
    let (a, b) = ([2.0, 0.0], [2.0, 3.0]);
    assert_eq!(crossings(BASE, seg(a, b)), Vec::<P2>::new(), "segments cross only strictly between their ends");
    assert_eq!(touch(BASE, seg(a, b)), Some([2.0, 0.0]), "the end is within the band of the other: a touch");
    let (plane, patch) = wall(0.0);
    let (counts, failures, found) = met(a, b, (&plane, patch));
    assert_eq!((counts, failures), ((0, 1, 0), vec![]), "the span ends on the surface: a contact");
    assert_eq!(found.contacts[0].t, 0.0, "{:?}", found.contacts);
}

#[test]
fn collinear_segments_that_overlap_touch_at_one_end_here_and_overlap_over_the_whole_span_there() {
    // From (2, 0) to (6, 0), along the base from its middle to 2 past its end: they share x from 2 to 4.
    let (a, b) = ([2.0, 0.0], [6.0, 0.0]);
    assert_eq!(crossings(BASE, seg(a, b)), Vec::<P2>::new(), "segments along one line cross nowhere");
    assert_eq!(touch(BASE, seg(a, b)), Some([4.0, 0.0]), "one place: the base's end, which lies on the other");
    let (plane, patch) = wall(0.0);
    let (counts, failures, found) = met(a, b, (&plane, patch));
    assert_eq!((counts, failures), ((0, 0, 1), vec![]), "no point, an overlap");
    assert_eq!(found.overlaps, [Overlap { lo: 0.0, hi: 4.0 }], "the whole span, though half of it is past the patch");
}

#[test]
fn an_end_within_the_band_past_the_other_s_end_is_a_touch_here_and_nothing_there() {
    // Upright across the base's line half the band past its end at (4, 0).
    let x = 4.0 + 0.5 * BAND;
    let (a, b) = ([x, -1.0], [x, 1.0]);
    assert_eq!(touch(BASE, seg(a, b)), Some([4.0, 0.0]), "the base's end is within the band of it: a touch");
    let (plane, patch) = wall(0.0);
    let (counts, failures, _) = met(a, b, (&plane, patch));
    assert_eq!((counts, failures), ((0, 0, 0), vec![]), "half the band past the patch's edge: no hit and no failure");
    let (plane, padded) = wall(BAND);
    let (counts, failures, found) = met(a, b, (&plane, padded));
    assert_eq!((counts, failures), ((1, 0, 0), vec![]), "a patch padded by the band holds it: a crossing");
    assert!((found.crossings[0].t - 1.0).abs() < 1e-9, "{:?}", found.crossings);
}

#[test]
fn a_gap_of_exactly_the_band_is_a_touch_here_and_undecided_there() {
    // An end the band above the base's middle. The distance is the square root of the band's square, which is the band
    // exactly, and `|q| <= band` is on (`Tolerance::band`).
    let over = |gap: f64| ([2.0, gap], [2.0, 3.0]);
    let (a, b) = over(BAND);
    assert_eq!(closest(BASE, seg(a, b)), (BAND, a), "the distance is the band, to the last bit");
    assert_eq!(touch(BASE, seg(a, b)), Some(a), "which is a touch");
    let (a, b) = over(2.0 * BAND);
    assert_eq!(touch(BASE, seg(a, b)), None, "twice the band is none");
    let (plane, patch) = wall(0.0);
    let (a, b) = over(BAND);
    let (counts, failures, _) = met(a, b, (&plane, patch));
    assert_eq!(counts, (0, 0, 0), "there the span's end is not decided to be on the surface");
    assert_eq!(failures, [Code::Undecided], "and that is reported, not guessed");
    let (a, b) = over(0.5 * BAND);
    let (counts, failures, _) = met(a, b, (&plane, patch));
    assert_eq!((counts, failures), ((0, 1, 0), vec![]), "half the band is a contact there, as it is a touch here");
    assert_eq!(touch(BASE, seg(a, b)), Some(a));
}
