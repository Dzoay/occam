//! Source: original
//! A seeded generated set of quadric pairs with no closed form, under rigid placements: spheres with cylinders, crossing
//! cylinders, spheres and cylinders with cones, and two cones apart, each with the answer known by an independent scan.
//! Along each ruling of one surface the other's implicit function is a quadratic, so the true section's points on 720
//! rulings are known, and its loops are the arcs of rulings that meet it, or two where every ruling does.

use std::f64::consts::TAU;
use std::sync::Arc;

use occam_geom::Surface;
use occam_math::sin_cos;
use occam_units::{Placement, Vec3, World};

use super::{Case, Seeded, check, cone_by, cylinder_by, sphere_along};
use crate::closed::tests::{O, Want, X, Z, v};

/// Where the line through `p` along `d` meets the zero set of the quadratic `f`.
fn hits(point: Vec3, along: Vec3, f: &dyn Fn(Vec3) -> f64) -> Vec<Vec3> {
    let (here, ahead, behind) = (f(point), f(point + along), f(point - along));
    let (bend, slope) = (f64::midpoint(ahead, behind) - here, 0.5 * (ahead - behind));
    let disc = slope * slope - 4.0 * bend * here;
    if disc <= 1e-12 || bend.abs() < 1e-12 {
        return Vec::new();
    }
    [1.0, -1.0].iter().map(|sign| point + along * ((-slope + sign * disc.sqrt()) / (2.0 * bend))).collect()
}

/// A family of lines, the rulings of one surface by their angle.
type Rulings = Box<dyn Fn(f64) -> (Vec3, Vec3)>;

/// What the scan of `rulings` against `f` finds: the loops, and points of the section on some of the rulings.
fn scanned(rulings: &Rulings, f: &dyn Fn(Vec3) -> f64) -> Want {
    let met: Vec<bool> = (0..720)
        .map(|i| {
            let (p, d) = rulings(TAU * f64::from(i) / 720.0);
            !hits(p, d, f).is_empty()
        })
        .collect();
    let starts = met.iter().zip(met.iter().cycle().skip(719)).filter(|(now, before)| **now && !**before).count();
    let loops = if met.iter().all(|m| *m) { 2 } else { starts };
    let on = (0..24)
        .flat_map(|i| {
            let (p, d) = rulings(TAU * (f64::from(i) + 0.37) / 24.0);
            hits(p, d, f)
        })
        .collect();
    Want { curves: loops, on, ..Want::default() }
}

fn case(name: String, [a, b]: [Arc<dyn Surface>; 2], want: Want, place: Placement<World, World>) -> Case {
    Case { name, a, b, want, place, unnamed: 0 }
}

/// A sphere with a cylinder through it, beside its centre by `e`.
fn sphere_cylinder(seed: &mut Seeded, i: u32) -> Case {
    let place = seed.rigid();
    let (radius, r) = (seed.next(1.2, 2.0), seed.next(0.3, 0.6));
    let e = seed.next(0.0, radius - 0.4 * r);
    let axis = v(seed.next(-1.0, 1.0), seed.next(-1.0, 1.0), seed.next(0.3, 1.0));
    let rulings: Rulings = Box::new(move |t| {
        let (sin, cos) = sin_cos(t);
        (v(e + r * cos, r * sin, 0.0), Z)
    });
    let want = scanned(&rulings, &move |p: Vec3| p.dot(p) - radius * radius);
    let surfaces = [cylinder_by(v(e, 0.0, 0.0), Z, r, &place), sphere_along(O, axis, radius, &place)];
    case(format!("sphere_cylinder[{i}]"), surfaces, want, place)
}

/// The cylinder of radius 1 about Z and a thinner one through `(0, e, 0)` whose axis crosses it at an angle.
fn crossing_cylinders(seed: &mut Seeded, index: u32) -> Case {
    let place = seed.rigid();
    let (rho, offset, psi) = (seed.next(0.3, 0.7), seed.next(0.0, 0.5), seed.next(-1.0, 1.0));
    let (sine, cosine) = sin_cos(psi);
    let along = v(cosine, 0.0, sine);
    let across = along.cross(v(0.0, 1.0, 0.0));
    let rulings: Rulings = Box::new(move |t| {
        let (s, c) = sin_cos(t);
        (v(0.0, offset + rho * c, 0.0) + across * (rho * s), along)
    });
    let want = scanned(&rulings, &|p: Vec3| p.x * p.x + p.y * p.y - 1.0);
    let surfaces = [cylinder_by(O, Z, 1.0, &place), cylinder_by(v(0.0, offset, 0.0), along, rho, &place)];
    case(format!("crossing_cylinders[{index}]"), surfaces, want, place)
}

/// The ruling of the cone about Z with its apex at the origin and half-angle `a`, at the angle `t`.
fn ruling(a: f64, t: f64) -> Vec3 {
    let ((sa, ca), (s, c)) = (sin_cos(a), sin_cos(t));
    v(sa * c, sa * s, ca)
}

/// The cone's implicit function, `ρ² cos² a − z² sin² a`.
fn cone_f(a: f64) -> impl Fn(Vec3) -> f64 {
    let (sa, ca) = sin_cos(a);
    move |p: Vec3| (p.x * p.x + p.y * p.y) * ca * ca - p.z * p.z * sa * sa
}

/// A cone with a sphere across it, beside its axis.
fn sphere_cone(seed: &mut Seeded, i: u32) -> Case {
    let place = seed.rigid();
    let (a, h) = (seed.next(0.35, 0.55), seed.next(1.5, 2.5));
    let radius = h * sin_cos(a).0 + seed.next(0.2, 0.4);
    let c = v(seed.next(0.0, 0.1), 0.0, h);
    let rulings: Rulings = Box::new(move |t| (O, ruling(a, t)));
    let want = scanned(&rulings, &move |p: Vec3| (p - c).dot(p - c) - radius * radius);
    let axis = v(seed.next(-1.0, 1.0), seed.next(-1.0, 1.0), seed.next(0.3, 1.0));
    let surfaces = [cone_by(O, Z, a, &place), sphere_along(c, axis, radius, &place)];
    case(format!("sphere_cone[{i}]"), surfaces, want, place)
}

/// A cone and a cylinder along X through its inside, at height `h`.
fn cylinder_cone(seed: &mut Seeded, index: u32) -> Case {
    let place = seed.rigid();
    let (half, height) = (seed.next(0.4, 0.6), seed.next(2.0, 3.0));
    let reach = height * sin_cos(half).0 / sin_cos(half).1;
    let (offset, radius) = (seed.next(0.0, 0.2) * reach, seed.next(0.2, 0.4) * reach);
    let rulings: Rulings = Box::new(move |t| {
        let (s, c) = sin_cos(t);
        (v(0.0, offset + radius * c, height + radius * s), X)
    });
    let want = scanned(&rulings, &cone_f(half));
    let surfaces = [cylinder_by(v(0.0, offset, height), X, radius, &place), cone_by(O, Z, half, &place)];
    case(format!("cylinder_cone[{index}]"), surfaces, want, place)
}

/// Two cones apart, the second narrower and leaning a little, so that they share no direction and their section is
/// bounded.
fn two_cones(seed: &mut Seeded, index: u32) -> Case {
    let place = seed.rigid();
    let (wide, lean) = (seed.next(0.5, 0.6), seed.next(0.0, 0.08));
    let narrow = seed.next(0.2, wide - lean - 0.1);
    let apex = v(seed.next(0.1, 0.4), 0.0, seed.next(1.5, 2.5));
    let (sl, cl) = sin_cos(lean);
    let (up, east) = (v(sl, 0.0, cl), v(cl, 0.0, -sl));
    let north = up.cross(east);
    let rulings: Rulings = Box::new(move |t| {
        let ((sb, cb), (s, c)) = (sin_cos(narrow), sin_cos(t));
        (apex, east * (sb * c) + north * (sb * s) + up * cb)
    });
    let want = scanned(&rulings, &cone_f(wide));
    let surfaces = [cone_by(O, Z, wide, &place), cone_by(apex, up, narrow, &place)];
    case(format!("two_cones[{index}]"), surfaces, want, place)
}

#[test]
fn a_seeded_generated_set_under_rigid_placements_is_intersected_exactly() {
    let mut seed = Seeded::new(5);
    let families: [fn(&mut Seeded, u32) -> Case; 5] =
        [sphere_cylinder, crossing_cylinders, sphere_cone, cylinder_cone, two_cones];
    let mut count = 0_u32;
    for i in 0..4 {
        for family in families {
            let case = family(&mut seed, i);
            assert!(case.want.curves > 0, "{}: the scan finds the section", case.name);
            check(&case);
            count = count.saturating_add(1);
        }
    }
    assert_eq!(count, 20);
}
