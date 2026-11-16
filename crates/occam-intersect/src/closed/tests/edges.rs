//! Source: original
//! The closed forms' edges that P5's cases do not reach: planes that do not share a point, a plane off a cylinder's
//! axis point, the three ways a plane through a cone's apex meets it, a parabola on the nappe below the apex, and the
//! order a pair of one kind is solved in.

use std::f64::consts::FRAC_PI_6;

use occam_geom::{Analytic, BatchKey, Content, FamilyName, Surface};
use occam_math::sin_cos;
use occam_units::Vec3;

use super::{O, Z, check, cone, contact, curves, cylinder, plane, tol, v};

fn unit(p: Vec3) -> Vec3 {
    p * (1.0 / p.norm())
}

#[test]
fn planes_that_share_no_point_meet_where_both_are() {
    // z = 0 and the plane through (0.3, 0.4, 0.5) facing (0, -0.8, 0.6) meet where y = 0.4 - 0.5 * 0.6 / 0.8.
    let other = plane(v(0.3, 0.4, 0.5), v(0.0, -0.8, 0.6));
    let y = 0.4 - 0.375;
    check("pp_apart", plane(O, Z).as_ref(), other.as_ref(), &curves(1, vec![v(-1.0, y, 0.0), v(2.0, y, 0.0)]));
    // And a plane slanting across a cylinder from a point off its axis: x + 2z = 1.7 meets it about z = 0.85.
    let slant = plane(v(0.3, -0.2, 0.7), v(1.0, 0.0, 2.0));
    let ring = (0..4).map(|i| {
        let (sin, cos) = sin_cos(f64::from(i));
        v(cos, sin, 0.85 - cos / 2.0)
    });
    check("pc_ellipse_off", slant.as_ref(), cylinder(O, Z, 1.0).as_ref(), &curves(1, ring.collect()));
}

#[test]
fn a_plane_through_a_cones_apex_meets_it_in_the_apex_one_ruling_or_two() {
    let (sin, cos) = sin_cos(FRAC_PI_6);
    let c = cone(O, Z, FRAC_PI_6);
    // Steeper than the side: the apex alone.
    check("apex_alone", plane(O, v(0.3, 0.0, 1.0)).as_ref(), c.as_ref(), &contact(O));
    // As steep, along the ruling (sin α, 0, cos α): that ruling.
    let along = v(sin, 0.0, cos);
    let touching = plane(O, v(cos, 0.0, -sin));
    check("apex_one", touching.as_ref(), c.as_ref(), &curves(1, vec![along * 2.0, along * -1.5]));
    // Less steep and tilted, facing (1, 0, 0.3): the two rulings at α from the axis across it.
    let gx = -0.3 * cos;
    let gy = (1.0 - cos * cos - gx * gx).sqrt();
    let rulings = vec![v(gx, gy, cos), v(gx, -gy, cos), v(-gx, gy, -cos) * 2.0, v(-gx, -gy, -cos) * 2.0];
    check("apex_two", plane(O, v(1.0, 0.0, 0.3)).as_ref(), c.as_ref(), &curves(2, rulings));
}

#[test]
fn a_parabola_on_the_nappe_below_the_apex_opens_down_it() {
    // The plane through (0, 0, -1) along the ruling (sin α, 0, cos α), facing (cos α, 0, -sin α): on the ruling at
    // angle u, `z = 1 / (cos u - 1)`, below the apex.
    let (sin, cos) = sin_cos(FRAC_PI_6);
    let k = sin / cos;
    let on = [1.0, 2.0, 3.0, 4.5].map(|u: f64| {
        let (su, cu) = sin_cos(u);
        let z = 1.0 / (cu - 1.0);
        v(k * z * cu, k * z * su, z)
    });
    let p = plane(v(0.0, 0.0, -1.0), v(cos, 0.0, -sin));
    check("pcone_parabola_below", p.as_ref(), cone(O, Z, FRAC_PI_6).as_ref(), &curves(1, on.to_vec()));
}

/// The direction of the one curve the closed forms find for `a` and `b`.
fn direction(a: &dyn Surface, b: &dyn Surface) -> Vec3 {
    let section = super::section(a, b);
    let [piece] = section.curves.as_slice() else { panic!("{} curves", section.curves.len()) };
    unit(piece.curve.first(0.0).1.rate())
}

/// What a surface is put in order by, as the method list orders a pair.
fn key(s: &dyn Surface) -> (FamilyName, BatchKey, Vec<u64>) {
    let mut content = Content::default();
    s.content(&mut content);
    (s.family(), s.batch_key(), content.words().to_vec())
}

#[test]
fn two_planes_meet_along_the_first_ones_normal_across_the_seconds() {
    let (one, other) = (plane(O, Z), plane(v(0.0, 1.0, 0.0), v(0.0, -0.8, 0.6)));
    let normal = |s: &dyn Surface| match s.analytic(tol()) {
        Some(Analytic::Plane { normal, .. }) => normal.get(),
        _ => panic!("a plane"),
    };
    let (first, second) = if key(one.as_ref()) > key(other.as_ref()) { (&other, &one) } else { (&one, &other) };
    let want = unit(normal(first.as_ref()).cross(normal(second.as_ref())));
    for (a, b) in [(&one, &other), (&other, &one)] {
        assert!((direction(a.as_ref(), b.as_ref()) - want).norm() <= 1e-12, "as the canonical order gives it");
    }
}

#[test]
fn cylinders_whose_axes_cross_square_on_the_worlds_axes_decline() {
    // Their axes' cross product lies along the world's z alone.
    let _unplaced = super::unplaced();
    super::declines("cc_square", cylinder(O, super::X, 1.0).as_ref(), cylinder(O, v(0.0, 1.0, 0.0), 0.7).as_ref());
}

#[test]
fn a_circle_cut_at_a_spheres_poles_keeps_its_image_continuous_however_it_lies() {
    // Planes through the sphere's axis at many turns, and through one pole at many tilts, placed and not: each arc's
    // image on the sphere keeps within a step of itself from end to end.
    for placed in [true, false] {
        let _unplaced = (!placed).then(super::unplaced);
        let s = super::sphere(O, 1.0);
        for k in 0..12 {
            let (sin, cos) = sin_cos(0.1 + f64::from(k) * FRAC_PI_6);
            let cases = [(plane(O, v(cos, sin, 0.0)), 2), (plane(Z, v(cos, sin, 0.8)), 1)];
            for (p, arcs) in cases {
                let got = super::section(p.as_ref(), s.as_ref());
                assert_eq!((got.curves.len(), got.failures.len()), (arcs, 0), "{k}, placed {placed}");
                for piece in &got.curves {
                    let range = piece.curve.range();
                    let at = |i: u32| piece.on_b.uv(range.lo + (range.hi - range.lo) * f64::from(i) / 400.0);
                    for i in 1..399 {
                        let (a, b) = (at(i), at(i + 1));
                        let jump = (a.u() - b.u()).abs() + (a.v() - b.v()).abs();
                        assert!(jump < 0.5, "{k}, placed {placed}: the image jumps by {jump} at step {i}");
                    }
                }
            }
        }
    }
}
