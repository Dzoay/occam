//! Source: original
//! Every degenerate class built in by construction, under seeded rigid placements, and classified as built: two equal
//! cylinders whose axes cross split into two ellipses, Viviani's sphere and cylinder meet in a nodal quartic, two cones
//! with one apex meet in their common rulings, and a cylinder tangent to a cone along a ruling meets it in that ruling
//! and an ellipse. Then what the band cannot decide, what the closed forms also answer, a swapped pair, and the
//! degenerate pencils this method reports rather than fits.

use std::f64::consts::PI;
use std::sync::Arc;

use occam_geom::{Cylinder, Surface};
use occam_math::{atan2, sin_cos};
use occam_report::{Code, Fault, Reason};
use occam_units::{Angle, Direction, Length, Placement, Tolerance};

use super::{Case, Class, Seeded, alone, both, check, class, cone_by, cylinder_by, sphere_along};
use crate::closed::tests::{O, SIZE, Want, X, Z, cone, cylinder, from_curve, nearer, section, sphere, v};
use crate::{Budget, Methods, Pencil, Section};

/// Two cylinders of radius `r` whose axes cross at the origin at the angle `psi`: two ellipses, in the planes through
/// the common normal Y and either bisector of the axes, crossing at `(0, ±r, 0)`, where each is cut. The relative
/// curvature there is the difference of the two cylinders' `1/r` across their axes, of eigenvalues `±sin psi / r`.
pub(super) fn equal_crossing(seed: &mut Seeded, index: u32) -> Case {
    let place = seed.rigid();
    let (radius, psi) = (seed.next(0.5, 1.2), seed.next(0.5, 1.4));
    let (sine, cosine) = sin_cos(psi);
    let along = v(sine, 0.0, cosine);
    let mut on = Vec::new();
    for bisector in [Z + along, Z - along] {
        let normal = v(0.0, 1.0, 0.0).cross(bisector);
        for k in 0..5 {
            let (s, c) = sin_cos(f64::from(k) + 0.4);
            on.push(v(radius * c, radius * s, -normal.x * radius * c / normal.z));
        }
    }
    let crossings = vec![v(0.0, radius, 0.0), v(0.0, -radius, 0.0)];
    let want = Want { curves: 4, contacts: crossings, on, ..Want::default() }.below(sine / radius);
    let (a, b) = (cylinder_by(O, Z, radius, &place), cylinder_by(O, along, radius, &place));
    Case { name: format!("equal_crossing[{index}]"), a, b, want, place, unnamed: 0 }
}

/// Viviani's pair at any size: the cylinder of radius `r` about the axis through `(r, 0, 0)` inside the sphere of radius
/// `2r` about the origin, whose own axis leans, so that the curve passes neither pole, and is cut at its node only.
pub(super) fn viviani(seed: &mut Seeded, index: u32) -> Case {
    let place = seed.rigid();
    let radius = seed.next(0.5, 1.0);
    let lean = v(seed.next(0.3, 1.0), seed.next(0.3, 1.0), seed.next(0.2, 0.6));
    let on = (0..9)
        .map(|k| {
            let turn = 4.0 * PI * (f64::from(k) + 0.3) / 9.0;
            let (s, c) = sin_cos(turn);
            v(radius + radius * c, radius * s, 2.0 * radius * sin_cos(0.5 * turn).0)
        })
        .collect();
    let want = Want { curves: 2, contacts: vec![v(2.0 * radius, 0.0, 0.0)], on, ..Want::default() }.below(0.5 / radius);
    let (a, b) = (cylinder_by(v(radius, 0.0, 0.0), Z, radius, &place), sphere_along(O, lean, 2.0 * radius, &place));
    Case { name: format!("viviani[{index}]"), a, b, want, place, unnamed: 0 }
}

/// Two cones with their apex at the origin, of half-angles `a` and `b`, their axes `psi` apart, less than `a + b` and
/// more than their difference: the circles of their directions cross twice, so they share two rulings, and the apex
/// is a contact.
pub(super) fn shared_apex(seed: &mut Seeded, i: u32) -> Case {
    let place = seed.rigid();
    let (a, b) = (seed.next(0.3, 0.6), seed.next(0.3, 0.6));
    let psi = seed.next((a - b).abs() + 0.1, a + b - 0.1);
    let (sin, cos) = sin_cos(psi);
    let want = Want { curves: 2, contacts: vec![O], ..Want::default() };
    let (first, second) = (cone_by(O, Z, a, &place), cone_by(O, v(sin, 0.0, cos), b, &place));
    Case { name: format!("shared_apex[{i}]"), a: first, b: second, want, place, unnamed: 0 }
}

/// A cone of half-angle `a` and a cylinder of radius `r` along its ruling in the XZ plane, tangent to it along that
/// ruling from outside or from inside: the ruling and an ellipse, which meets the ruling at a contact.
pub(super) fn tangent_along_a_ruling(seed: &mut Seeded, index: u32) -> Case {
    let place = seed.rigid();
    let (half, radius) = (seed.next(0.3, 0.6), seed.next(0.3, 1.0));
    let (sine, cosine) = sin_cos(half);
    let (ruling, normal) = (v(sine, 0.0, cosine), v(cosine, 0.0, -sine));
    let outward = if index.is_multiple_of(2) { 1.0 } else { -1.0 };
    let on = (0..5).map(|k| ruling * (f64::from(k) - 2.0)).collect();
    let want = Want { curves: 2, on, ..Want::default() };
    let (tube, cone) = (cylinder_by(normal * (outward * radius), ruling, radius, &place), cone_by(O, Z, half, &place));
    Case { name: format!("tangent_along_a_ruling[{index}]"), a: tube, b: cone, want, place, unnamed: 1 }
}

#[test]
fn every_degenerate_class_is_built_and_classified_as_built_and_none_is_fitted() {
    let mut seed = Seeded::new(11);
    for i in 0..6 {
        let split = equal_crossing(&mut seed, i);
        assert!(matches!(class(split.a.as_ref(), split.b.as_ref()), Class::Split { .. }), "{}", split.name);
        check(&split);
        let node = viviani(&mut seed, i);
        assert!(matches!(class(node.a.as_ref(), node.b.as_ref()), Class::Node { .. }), "{}", node.name);
        check(&node);
        let lines = shared_apex(&mut seed, i);
        assert_eq!(class(lines.a.as_ref(), lines.b.as_ref()), Class::Lines, "{}", lines.name);
        check(&lines);
        let ruling = tangent_along_a_ruling(&mut seed, i);
        assert!(matches!(class(ruling.a.as_ref(), ruling.b.as_ref()), Class::Split { .. }), "{}", ruling.name);
        check(&ruling);
    }
}

/// The cylinders of radius 1 about Z and of radius `1 + 2⁻⁴⁰` about X, neither placed, so that where they come nearest
/// their gap is `2⁻⁴⁰` exactly before rounding, decided at the band `band`.
fn nudged(band: f64) -> Section {
    let across = Placement::rotation(Direction::normalised(v(0.0, 1.0, 0.0)), Angle::from_radians(0.5 * PI));
    let mm = Length::from_millimetres;
    let a: Arc<dyn Surface> = Arc::new(Cylinder::placed(&Placement::identity(), mm(1.0)).unwrap());
    let b: Arc<dyn Surface> =
        Arc::new(Cylinder::placed(&Placement::identity().then(&across), mm(1.0 + NUDGE)).unwrap());
    let tol = Tolerance::new(mm(band), Angle::from_radians(1e-12)).unwrap();
    Methods::new(vec![Box::new(Pencil)]).section(a.as_ref(), b.as_ref(), tol, Budget { steps: 4 })
}

/// 2⁻⁴⁰.
const NUDGE: f64 = 1.0 / 1_099_511_627_776.0;

#[test]
fn a_pencil_nudged_to_the_edge_of_the_band_of_a_split_is_undecided_and_nothing_is_guessed() {
    let got = nudged(NUDGE);
    assert_eq!((got.method, got.curves.len(), got.contacts.len()), (Some("pencil"), 0, 0));
    let [report] = got.failures.as_slice() else { panic!("{:?}", got.failures) };
    assert_eq!((report.code, report.fault), (Code::Undecided, Some(Fault::NumericallySensitive)));
    assert!(got.tally.undecided >= 1, "{:?}", got.tally);
    let split = nudged(2.0 * NUDGE);
    assert!(split.failures.is_empty(), "{:?}", split.failures);
    assert_eq!((split.curves.len(), split.contacts.len()), (4, 2), "within the band, a split, cut where it crosses");
    let apart = nudged(0.5 * NUDGE);
    assert!(apart.failures.is_empty(), "{:?}", apart.failures);
    assert_eq!((apart.curves.len(), apart.contacts.len()), (2, 0), "clear of it, two loops");
}

/// How far the farthest sampled point of `from`'s curves is from the nearest of `to`'s.
fn farthest(from: &Section, to: &Section) -> f64 {
    let mut worst: f64 = 0.0;
    for piece in &from.curves {
        let range = piece.curve.range();
        let (lo, hi) = if range.lo.is_finite() { (range.lo, range.hi) } else { (-4.0, 4.0) };
        for k in 0..=32 {
            let p = piece.curve.point(lo + (hi - lo) * f64::from(k) / 32.0).to_vec3();
            let near = to.curves.iter().map(|c| from_curve(c.curve.as_ref(), p)).fold(f64::INFINITY, nearer);
            worst = if near > worst { near } else { worst };
        }
    }
    worst
}

#[test]
fn where_a_closed_form_also_answers_the_two_agree_within_the_band() {
    let pairs = [
        ("sc_two_circles", sphere(O, 1.0), cylinder(O, Z, 0.6)),
        ("cone_cyl_coaxial", cone(O, Z, atan2(0.5, 1.0)), cylinder(O, Z, 1.0)),
        ("cone_sphere", cone(O, Z, PI / 6.0), sphere(v(0.0, 0.0, 2.0), 1.2)),
    ];
    for (name, a, b) in pairs {
        let (closed, pencil) = (section(a.as_ref(), b.as_ref()), alone(a.as_ref(), b.as_ref()));
        assert_eq!((closed.method, pencil.method), (Some("closed"), Some("pencil")), "{name}");
        assert!(pencil.failures.is_empty(), "{name}: {:?}", pencil.failures);
        assert_eq!(closed.curves.len(), pencil.curves.len(), "{name}");
        let (there, back) = (farthest(&pencil, &closed), farthest(&closed, &pencil));
        assert!(there <= 1e-12 * SIZE && back <= 1e-12 * SIZE, "{name}: {there} and {back} apart");
    }
}

#[test]
fn swapping_a_pair_gives_the_same_section_with_its_images_swapped() {
    let pairs = [
        ("viviani", cylinder(X, Z, 1.0), sphere(O, 2.0)),
        ("perp_unequal", cylinder(O, Z, 1.0), cylinder(O, X, 0.6)),
        ("steinmetz", cylinder(O, Z, 1.0), cylinder(O, X, 1.0)),
    ];
    for (name, a, b) in pairs {
        let (one, other) = (both(a.as_ref(), b.as_ref()), both(b.as_ref(), a.as_ref()));
        assert_eq!((one.curves.len(), one.contacts.len()), (other.curves.len(), other.contacts.len()), "{name}");
        assert!(!one.curves.is_empty(), "{name}");
        for (x, y) in one.curves.iter().zip(&other.curves) {
            let range = x.curve.range();
            for k in 1..8 {
                let t = range.lo + (range.hi - range.lo) * f64::from(k) / 8.0;
                assert_eq!(x.curve.point(t), y.curve.point(t), "{name}");
                assert_eq!((x.on_a.uv(t), x.on_b.uv(t)), (y.on_b.uv(t), y.on_a.uv(t)), "{name}");
            }
        }
    }
}

#[test]
fn a_degenerate_pencil_this_method_does_not_parametrize_is_reported_and_not_fitted() {
    let (sa, ca) = sin_cos(0.4);
    let (g, n) = (v(sa, 0.0, ca), v(ca, 0.0, -sa));
    // Axes 0.7 apart, less than the two half-angles together, so that the circles of their directions cross.
    let leaning = v(sin_cos(0.7).0, 0.0, sin_cos(0.7).1);
    let cases = [
        ("node_at_infinity", cylinder(n * 2.0, g, 0.5), cone(O, Z, 0.4), "a quartic with its node at infinity"),
        ("reaches_infinity", cone(O, Z, 0.5), cone(v(2.0, 0.0, 0.0), leaning, 0.5), "a quartic that reaches infinity"),
    ];
    for (name, a, b, why) in cases {
        assert_eq!(class(a.as_ref(), b.as_ref()), Class::Other(why), "{name}");
        let got = both(a.as_ref(), b.as_ref());
        assert_eq!((got.method, got.curves.len()), (Some("pencil"), 0), "{name}: answered, so not fitted");
        let [report] = got.failures.as_slice() else { panic!("{name}: {:?}", got.failures) };
        let kind = (report.code, report.fault, report.reason);
        assert_eq!(kind, (Code::Incomplete, Some(Fault::NotImplemented), Some(Reason::PairNotSupported)), "{name}");
    }
}
