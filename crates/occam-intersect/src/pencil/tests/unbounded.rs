//! Source: original
//! The sections that reach infinity, each built by construction under seeded rigid placements and checked at every
//! scale a part may have. Two cones that share ruling directions meet in a smooth quartic that runs to infinity along
//! them, and in a nodal one where one's apex is on the other. A cylinder along a ruling direction of a cone meets it in
//! a quartic with its node at infinity, which branches reach or none does. The same cylinder through the cone's apex
//! meets it in that ruling and a cubic, crossing at the apex.
//!
//! Each is given within the extent asked. Every point of the true section in the extent lies on a curve, and every curve
//! lies on both surfaces within the band, with its images mapping onto it. A curve ends only outside the extent, at a
//! cone's apex, or where another ends. Nothing is reported as not implemented, and swapping the pair swaps the images.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use occam_geom::{Surface, Wrap};
use occam_math::{sin_cos, smaller};
use occam_units::{Bounds, Placement, Point, Vec3, World};

use super::scale::{SCALES, grown, kind};
use super::{Seeded, class, cone_by, cylinder_by, moved};
use crate::closed::tests::{O, Z, at_scale, contact_maps, from_curve, lies_on_both, mm, samples, scale, size, tol, v};
use crate::{Budget, Closed, Contact, Methods, Pencil, Section};

const Y: Vec3 = v(0.0, 1.0, 0.0);

/// How far the extent asked reaches from a case's middle in each coordinate, in units of the case's own frame: its
/// surfaces are about four across.
const REACH: f64 = 3.0;

/// A pair whose section reaches infinity, built in its own frame and placed by `place`.
struct Reaching {
    name: String,
    a: Arc<dyn Surface>,
    b: Arc<dyn Surface>,
    place: Placement<World, World>,
    /// The class it is built as.
    kind: &'static str,
    /// Points of its true section, in its own frame.
    on: Vec<Vec3>,
    /// Its cones' apexes, where an image is undefined and a curve may end, in its own frame.
    apexes: Vec<Vec3>,
    /// Its contacts, in its own frame.
    contacts: Vec<Vec3>,
}

/// The value at `p` of the cone with its apex at `apex`, opening along the unit `axis` by `half`: zero on it.
fn on_cone(apex: Vec3, axis: Vec3, half: f64) -> impl Fn(Vec3) -> f64 {
    let cos = sin_cos(half).1;
    move |p| {
        let d = p - apex;
        cos * cos * d.dot(d) - d.dot(axis) * d.dot(axis)
    }
}

/// Where the line from `from` along the unit `along` meets the zero set of the quadratic `value`, as distances along
/// it: `value` along the line is `a t² + b t + c`, read from its values at -1, 0 and 1, and a line along which it is linear
/// meets it once.
fn meets(from: Vec3, along: Vec3, value: &impl Fn(Vec3) -> f64) -> Vec<f64> {
    let (constant, up, down) = (value(from), value(from + along), value(from - along));
    let (square, linear) = (f64::midpoint(up, down) - constant, 0.5 * (up - down));
    if square.abs() <= 1e-12 * (linear.abs() + constant.abs()) {
        return vec![-constant / linear];
    }
    let disc = linear * linear - 4.0 * square * constant;
    if disc < 0.0 {
        return Vec::new();
    }
    let q = -0.5 * (linear + disc.sqrt().copysign(linear));
    vec![q / square, constant / q]
}

/// The rulings' directions of the cone about Z opening by `half`, at 48 angles.
fn directions(half: f64) -> impl Iterator<Item = Vec3> {
    let (sh, ch) = sin_cos(half);
    (0..48).map(move |i| {
        let (s, c) = sin_cos(std::f64::consts::TAU * (f64::from(i) + 0.3) / 48.0);
        v(sh * c, sh * s, ch)
    })
}

/// The points where the cone about Z from the origin opening by `half` meets `value` along its rulings, its apex
/// aside.
fn from_apex(half: f64, value: &impl Fn(Vec3) -> f64) -> Vec<Vec3> {
    directions(half)
        .flat_map(|g| meets(O, g, value).into_iter().filter(|t| t.abs() > 1e-9).map(move |t| g * t))
        .collect()
}

/// The points where the cylinder of radius `radius` about the axis through `at` along the unit `g`, with `n` and `Y`
/// across it, meets `value` along its rulings, those within 0.02 of the angle `skip` aside.
fn on_rulings(at: Vec3, (g, n): (Vec3, Vec3), radius: f64, skip: f64, value: &impl Fn(Vec3) -> f64) -> Vec<Vec3> {
    let mut out = Vec::new();
    for i in 0..48 {
        let theta = std::f64::consts::TAU * (f64::from(i) + 0.3) / 48.0;
        if sin_cos(theta - skip).1 > sin_cos(0.02).1 {
            continue;
        }
        let (sine, cosine) = sin_cos(theta);
        let from = at + (n * cosine + Y * sine) * radius;
        out.extend(meets(from, g, value).into_iter().map(|t| from + g * t));
    }
    out
}

/// Two cones of half-angle 0.5, about Z from the origin and from `(2, 0, 0)` about the axis 0.7 from Z, whose circles of
/// directions cross twice: a smooth quartic that runs to infinity along both shared directions.
fn shared_directions(seed: &mut Seeded) -> Reaching {
    let place = seed.rigid();
    let (s, c) = sin_cos(0.7);
    let (apex, axis) = (v(2.0, 0.0, 0.0), v(s, 0.0, c));
    let on = from_apex(0.5, &on_cone(apex, axis, 0.5));
    let (a, b) = (cone_by(O, Z, 0.5, &place), cone_by(apex, axis, 0.5, &place));
    let (name, apexes) = ("shared_directions".to_string(), vec![O, apex]);
    Reaching { name, a, b, place, kind: "reaching", on, apexes, contacts: Vec::new() }
}

/// The cone of half-angle 0.5 about Z from the origin, and a narrow one, of half-angle 0.3 from `(3, 0, 0)` about the
/// axis 0.7 from Z, which most of the first's rulings miss: the quartic's loops lie between roots of its discriminant,
/// and run to infinity along the two directions the cones share.
fn shared_directions_apart(seed: &mut Seeded) -> Reaching {
    let place = seed.rigid();
    let (s, c) = sin_cos(0.7);
    let (apex, axis) = (v(3.0, 0.0, 0.0), v(s, 0.0, c));
    let on = from_apex(0.5, &on_cone(apex, axis, 0.3));
    let (a, b) = (cone_by(O, Z, 0.5, &place), cone_by(apex, axis, 0.3, &place));
    let (name, apexes) = ("shared_directions_apart".to_string(), vec![O, apex]);
    Reaching { name, a, b, place, kind: "reaching", on, apexes, contacts: Vec::new() }
}

/// The cone of half-angle 0.5 about Z from the origin, and the cone from `(0, 0, -2)` about the axis 0.45 from Z
/// opening by 0.45, whose ruling along Z passes through the first's apex and whose directions cross the first's: a
/// nodal quartic, its node at that apex, that runs to infinity.
fn nodal_shared_directions(seed: &mut Seeded) -> Reaching {
    let place = seed.rigid();
    let (s, c) = sin_cos(0.45);
    let (apex, axis) = (v(0.0, 0.0, -2.0), v(s, 0.0, c));
    let on = from_apex(0.5, &on_cone(apex, axis, 0.45));
    let (a, b) = (cone_by(O, Z, 0.5, &place), cone_by(apex, axis, 0.45, &place));
    let (name, apexes) = ("nodal_shared_directions".to_string(), vec![O, apex]);
    Reaching { name, a, b, place, kind: "node", on, apexes, contacts: vec![O] }
}

/// The cone of half-angle 0.4 about Z from the origin, its ruling `g` in the XZ plane and its normal `n` there, and the
/// cylinder of radius 0.5 along `g` about the axis through `at`, whose rulings run along the cone's asymptotic cone: a
/// quartic with its node at infinity, which two branches reach where the axis is nearer the cone's tangent plane along
/// `g` than the radius, and none does where it is farther.
fn along_a_ruling(seed: &mut Seeded, name: &str, at: Vec3, kind: &'static str, contacts: Vec<Vec3>) -> Reaching {
    let place = seed.rigid();
    let (sa, ca) = sin_cos(0.4);
    let (g, n) = (v(sa, 0.0, ca), v(ca, 0.0, -sa));
    let skip = if contacts.is_empty() { f64::NAN } else { -0.5 * std::f64::consts::PI };
    let mut on = on_rulings(at, (g, n), 0.5, skip, &on_cone(O, Z, 0.4));
    if !contacts.is_empty() {
        on.extend((0..12).map(|i| g * (f64::from(i) * 0.5 - 2.75)));
    }
    let (a, b) = (cylinder_by(at, g, 0.5, &place), cone_by(O, Z, 0.4, &place));
    Reaching { name: name.to_string(), a, b, place, kind, on, apexes: vec![O], contacts }
}

/// The quartic with its node at infinity, crossed there by two branches.
fn node_at_infinity(seed: &mut Seeded) -> Reaching {
    let n = v(sin_cos(0.4).1, 0.0, -sin_cos(0.4).0);
    along_a_ruling(seed, "node_at_infinity", n * 0.25 + Y, "node at infinity", Vec::new())
}

/// The quartic with its node at infinity, which no branch reaches: a bounded loop.
fn isolated_at_infinity(seed: &mut Seeded) -> Reaching {
    let n = v(sin_cos(0.4).1, 0.0, -sin_cos(0.4).0);
    along_a_ruling(seed, "isolated_at_infinity", n * 2.0, "node at infinity", Vec::new())
}

/// The cylinder along the ruling `g` through the cone's apex: that ruling and a cubic, which crosses it at the apex.
fn ruling_and_cubic(seed: &mut Seeded) -> Reaching {
    along_a_ruling(seed, "ruling_and_cubic", Y * 0.5, "ruling and cubic", vec![O])
}

/// The closed forms, then the pencil method within `extent`.
fn within(a: &dyn Surface, b: &dyn Surface, extent: Bounds) -> Section {
    Methods::new(vec![Box::new(Closed), Box::new(Pencil::within(extent))]).section(a, b, tol(), Budget { steps: 16 })
}

/// The extent asked of `case`: `reach` from its middle in each coordinate, at this thread's scale.
fn extent_of(case: &Reaching, reach: f64) -> Bounds {
    Bounds::around(Point::from_vec3(moved(&case.place, O)), mm(reach * scale()))
}

/// The ends of each open curve of `got` with finite ends.
fn ends(got: &Section) -> Vec<Vec3> {
    let open = got.curves.iter().map(|c| (c.curve.as_ref(), c.curve.range()));
    let finite = open.filter(|(_, r)| r.wrap == Wrap::Open && r.lo.is_finite() && r.hi.is_finite());
    finite.flat_map(|(c, r)| [c.point(r.lo).to_vec3(), c.point(r.hi).to_vec3()]).collect()
}

/// `case`'s section within `reach`, as the module says it is.
fn check(case: &Reaching, reach: f64) {
    let (name, a, b, extent) = (&case.name, case.a.as_ref(), case.b.as_ref(), extent_of(case, reach));
    let got = within(a, b, extent);
    assert_eq!(got.method, Some("pencil"), "{name}: {:?}", got.failures);
    assert!(got.failures.is_empty(), "{name}: {:?}", got.failures);
    assert!(!got.curves.is_empty(), "{name}: no curve");
    for piece in &got.curves {
        lies_on_both(name, piece, a, b);
    }
    assert_eq!(got.contacts.len(), case.contacts.len(), "{name}: {:?}", got.contacts);
    for p in case.contacts.iter().map(|p| moved(&case.place, *p)) {
        let near = |c: &Contact| matches!(c, Contact::Point { at, .. } if (at.to_vec3() - p).norm() <= 1e-10 * size());
        assert!(got.contacts.iter().any(near), "{name}: no contact at {p:?} in {:?}", got.contacts);
    }
    for found in &got.contacts {
        contact_maps(name, found, a, b);
    }
    let on = case.on.iter().map(|p| moved(&case.place, *p));
    let inside: Vec<Vec3> = on.filter(|p| extent.contains(Point::from_vec3(*p))).collect();
    assert!(inside.len() >= 8, "{name}: only {} points of the section in the extent", inside.len());
    for p in inside {
        let near = got.curves.iter().map(|c| from_curve(c.curve.as_ref(), p)).fold(f64::INFINITY, smaller);
        assert!(near <= 1e-9 * size(), "{name}: no curve passes through {p:?}: the nearest is {near} away");
    }
    let (ends, apexes) = (ends(&got), case.apexes.iter().map(|p| moved(&case.place, *p)).collect::<Vec<_>>());
    for (i, end) in ends.iter().enumerate() {
        let meets = |p: &Vec3| (*p - *end).norm() <= 1e-9 * size();
        let joined = ends.iter().enumerate().any(|(j, other)| j != i && meets(other));
        let free = extent.contains(Point::from_vec3(*end)) && !joined && !apexes.iter().any(meets);
        assert!(!free, "{name}: a curve ends at {end:?}, inside the extent, at no apex and no other end");
    }
}

/// `case` swapped gives the same curves, point for point, with their images swapped.
fn swaps(case: &Reaching) {
    let (name, a, b, extent) = (&case.name, case.a.as_ref(), case.b.as_ref(), extent_of(case, REACH));
    let (one, other) = (within(a, b, extent), within(b, a, extent));
    assert_eq!((one.curves.len(), one.contacts.len()), (other.curves.len(), other.contacts.len()), "{name}");
    for (x, y) in one.curves.iter().zip(&other.curves) {
        for t in samples(x.curve.range()).into_iter().step_by(8) {
            assert_eq!(x.curve.point(t), y.curve.point(t), "{name}");
            assert_eq!((x.on_a.uv(t), x.on_b.uv(t)), (y.on_b.uv(t), y.on_a.uv(t)), "{name}");
        }
    }
}

/// The cases `made` gives that fail at some scale, each with why.
fn failing(made: fn(&mut Seeded) -> Vec<Reaching>) -> Vec<String> {
    let mut out = Vec::new();
    for k in SCALES {
        let _scale = at_scale(k);
        for case in made(&mut Seeded::new(29)) {
            let case = Reaching { a: grown(&case.a, k), b: grown(&case.b, k), ..case };
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                assert_eq!(kind(&class(case.a.as_ref(), case.b.as_ref())), case.kind, "classified");
                check(&case, REACH);
                swaps(&case);
            }));
            if let Err(why) = outcome {
                let why = why.downcast_ref::<String>().cloned().unwrap_or_default();
                out.push(format!("{} at {k}: {}", case.name, why.lines().next().unwrap_or("")));
            }
        }
    }
    out
}

#[test]
fn two_cones_that_share_ruling_directions_meet_in_branches_given_within_the_extent_at_every_scale() {
    let failures =
        failing(|seed| vec![shared_directions(seed), shared_directions_apart(seed), nodal_shared_directions(seed)]);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_quartic_with_its_node_at_infinity_is_given_within_the_extent_at_every_scale() {
    let failures = failing(|seed| vec![node_at_infinity(seed), isolated_at_infinity(seed)]);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_ruling_and_a_cubic_are_given_within_the_extent_crossing_at_the_apex_at_every_scale() {
    let failures = failing(|seed| vec![ruling_and_cubic(seed)]);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_wider_extent_gives_the_branches_further() {
    let mut seed = Seeded::new(31);
    let cases = [shared_directions(&mut seed), node_at_infinity(&mut seed), ruling_and_cubic(&mut seed)];
    for case in &cases {
        check(case, 4.0 * REACH);
        let reach = |r: f64| {
            let got = within(case.a.as_ref(), case.b.as_ref(), extent_of(case, r));
            ends(&got)
                .iter()
                .map(|e| (*e - moved(&case.place, O)).norm())
                .fold(0.0, |a: f64, d| if d > a { d } else { a })
        };
        let (near, far) = (reach(REACH), reach(4.0 * REACH));
        assert!(far > 3.0 * near && near > REACH, "{}: ends {near} and {far} from its middle", case.name);
    }
}
