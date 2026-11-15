//! Source: original
//! P5's closed-form cases (`docs/evidence/2026-11-09-p5.md`): its 34 analytic pairs with a closed form and the 24 of
//! its near-tangent sweep, rebuilt with the answers its spike knew by construction. Six of the 34 are cylinders whose
//! axes cross, which this step leaves to Levin's pencil method (`intersection.quadrics`), so they decline here, with
//! Viviani's pair. Two pairs P5 sent to the hybrid, a cone and a sphere each with a coaxial cylinder, are coaxial
//! surfaces of revolution, which the closed forms now answer.

use std::f64::consts::{FRAC_PI_6, PI};

use occam_math::{atan2, sin_cos};

use occam_units::Vec3;

use super::{
    Case, Coincidence, O, X, Z, case, check, circle, coincident, cone, contact, curves, cylinder, declines, nothing,
    plane, sphere, v,
};

/// The tangent, from the owned `sin_cos`.
fn tan(x: f64) -> f64 {
    let (sin, cos) = sin_cos(x);
    sin / cos
}

/// A point of P5's cone of slope `k = tan(π/6)` cut by the plane `n · p = d`, on the ruling at angle `u`.
fn on_cone(n: Vec3, d: f64, u: f64) -> Vec3 {
    let k = tan(FRAC_PI_6);
    let (sin, cos) = sin_cos(u);
    let height = d / (n.z + k * (n.x * cos + n.y * sin));
    v(k * height * cos, k * height * sin, height)
}

fn planes_and_spheres() -> Vec<Case> {
    let (flat, s) = (plane(O, Z), sphere(O, 1.0));
    let r = 0.91_f64.sqrt();
    vec![
        case("pp_cross", flat.clone(), plane(O, v(0.0, -0.8, 0.6)), curves(1, vec![v(-1.5, 0.0, 0.0), X])),
        case("pp_parallel", flat.clone(), plane(v(0.0, 0.0, 0.7), Z), nothing()),
        case("pp_coincident", flat, plane(v(0.3, 0.1, 0.0), -Z), coincident(Coincidence::Coplanar)),
        case("ps_circle", plane(v(0.0, 0.0, 0.3), Z), s.clone(), curves(1, circle(v(0.0, 0.0, 0.3), Z, r, 5))),
        case("ps_tangent", plane(Z, Z), s.clone(), contact(Z)),
        case("ps_disjoint", plane(v(0.0, 0.0, 1.5), Z), s, nothing()),
    ]
}

fn planes_and_cylinders() -> Vec<Case> {
    let c = cylinder(O, Z, 1.0);
    let ellipse = (0..5).map(|i| {
        let (sin, cos) = sin_cos(f64::from(i));
        v(cos, sin, -cos / 2.0)
    });
    let yy = 0.91_f64.sqrt();
    let lines = vec![v(0.3, yy, -2.0), v(0.3, yy, 1.0), v(0.3, -yy, -2.0), v(0.3, -yy, 1.0)];
    vec![
        case("pc_ellipse", plane(O, v(1.0, 0.0, 2.0)), c.clone(), curves(1, ellipse.collect())),
        case("pc_circle", plane(v(0.0, 0.0, 0.7), Z), c.clone(), curves(1, circle(v(0.0, 0.0, 0.7), Z, 1.0, 5))),
        case("pc_two_lines", plane(v(0.3, 0.0, 0.0), X), c.clone(), curves(2, lines)),
        case("pc_tangent_line", plane(X, X), c.clone(), curves(1, vec![v(1.0, 0.0, -2.0), v(1.0, 0.0, 2.0)])),
        case("pc_disjoint", plane(v(1.2, 0.0, 0.0), X), c, nothing()),
    ]
}

fn planes_and_cones() -> Vec<Case> {
    let c = cone(O, Z, FRAC_PI_6);
    let k = tan(FRAC_PI_6);
    let tilted = v(-0.2, 0.0, 1.0) * (1.0 / 1.04_f64.sqrt());
    let ruling = v(k, 0.0, 1.0) * (1.0 / (1.0 + k * k).sqrt());
    let along = v(ruling.z, 0.0, -ruling.x);
    let side = X;
    let sample = |n: Vec3, d: f64, us: &[f64]| us.iter().map(|u| on_cone(n, d, *u)).collect::<Vec<_>>();
    let apex_lines = vec![v(0.0, -1.5 * k, -1.5), v(0.0, 1.5 * k, 1.5), v(0.0, 1.5 * k, -1.5), v(0.0, -1.5 * k, 1.5)];
    vec![
        case("pcone_circle", plane(Z, Z), c.clone(), curves(1, circle(Z, Z, k, 5))),
        case("pcone_ellipse", plane(Z, tilted), c.clone(), curves(1, sample(tilted, tilted.z, &[0.0, 1.0, 2.0, 4.0]))),
        case("pcone_parabola", plane(Z, along), c.clone(), curves(1, sample(along, along.z, &[1.0, 2.0, 3.0, 4.0]))),
        case(
            "pcone_hyperbola",
            plane(v(0.4, 0.0, 0.0), side),
            c.clone(),
            curves(2, sample(side, 0.4, &[0.3, PI + 0.3])),
        ),
        case("pcone_lines_apex", plane(O, X), c.clone(), curves(2, apex_lines)),
        case("pcone_apex_point", plane(O, Z), c, contact(O)),
    ]
}

fn spheres() -> Vec<Case> {
    let s = sphere(O, 1.0);
    let a0: f64 = (1.21 + 1.0 - 1.44) / 2.2;
    let h = (1.0 - a0 * a0).sqrt();
    vec![
        case("ss_circle", s.clone(), sphere(v(1.1, 0.0, 0.0), 1.2), curves(1, circle(v(a0, 0.0, 0.0), X, h, 5))),
        case("ss_tangent_ext", s.clone(), sphere(v(2.2, 0.0, 0.0), 1.2), contact(X)),
        case("ss_tangent_int", s.clone(), sphere(v(0.3, 0.0, 0.0), 0.7), contact(X)),
        case("ss_disjoint", s.clone(), sphere(v(3.0, 0.0, 0.0), 1.2), nothing()),
        case("ss_contained", s.clone(), sphere(v(0.1, 0.0, 0.0), 0.5), nothing()),
        case("ss_coincident", s.clone(), sphere(O, 1.0), coincident(Coincidence::Coaxial)),
    ]
}

fn cylinders() -> Vec<Case> {
    let c = cylinder(O, Z, 1.0);
    let (d, r) = (1.2_f64, 0.9_f64);
    let a0 = (d * d + 1.0 - r * r) / (2.0 * d);
    let h = (1.0 - a0 * a0).sqrt();
    let lines = vec![v(a0, h, -2.0), v(a0, h, 2.0), v(a0, -h, -2.0), v(a0, -h, 2.0)];
    vec![
        case("cc_coaxial_same", c.clone(), cylinder(v(0.0, 0.0, 2.0), -Z, 1.0), coincident(Coincidence::Coaxial)),
        case("cc_coaxial_diff", c.clone(), cylinder(O, Z, 0.8), nothing()),
        case("cc_parallel_two_lines", c.clone(), cylinder(v(d, 0.0, 0.0), Z, r), curves(2, lines)),
        case(
            "cc_parallel_tangent",
            c.clone(),
            cylinder(v(1.7, 0.0, 0.0), Z, 0.7),
            curves(1, vec![X, v(1.0, 0.0, 2.0)]),
        ),
        case("cc_parallel_disjoint", c, cylinder(v(2.0, 0.0, 0.0), Z, 0.5), nothing()),
    ]
}

/// The two pairs P5 had no closed form for and sent to the hybrid, which are coaxial surfaces of revolution.
fn coaxial() -> Vec<Case> {
    let slope = atan2(0.5, 1.0);
    let rings = |z: f64, r: f64| [circle(v(0.0, 0.0, z), Z, r, 3), circle(v(0.0, 0.0, -z), Z, r, 3)].concat();
    vec![
        case("cone_cyl_coaxial", cone(O, Z, slope), cylinder(O, Z, 1.0), curves(2, rings(2.0, 1.0))),
        case("sc_two_circles", sphere(O, 1.0), cylinder(O, Z, 0.6), curves(2, rings(0.8, 0.6))),
    ]
}

#[test]
fn p5s_closed_form_pairs_lie_on_both_surfaces_within_1e_12_of_the_parts_size() {
    let groups = [planes_and_spheres(), planes_and_cylinders(), planes_and_cones(), spheres(), cylinders(), coaxial()];
    let cases: Vec<Case> = groups.into_iter().flatten().collect();
    assert_eq!(cases.len(), 28 + 2, "P5's 34 but the six whose axes cross, and the two that are coaxial");
    for (name, a, b, want) in &cases {
        check(name, a.as_ref(), b.as_ref(), want);
    }
}

#[test]
fn p5s_near_tangent_sweep_is_told_from_a_tangency_down_to_a_gap_of_1e_12() {
    let gaps = [1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12];
    for eps in gaps {
        let d: f64 = 2.0 - eps;
        let a0 = d / 2.0;
        let h = ((1.0 - a0) * (1.0 + a0)).sqrt();
        let ring = circle(v(a0, 0.0, 0.0), X, h, 4);
        check(
            &format!("nt_ss_circle[{eps:e}]"),
            sphere(O, 1.0).as_ref(),
            sphere(v(d, 0.0, 0.0), 1.0).as_ref(),
            &curves(1, ring),
        );
        let x = 1.0 - eps;
        let y = ((1.0 - x) * (1.0 + x)).sqrt();
        let lines = curves(2, vec![v(x, y, -2.0), v(x, y, 2.0), v(x, -y, -2.0), v(x, -y, 2.0)]);
        check(
            &format!("nt_pc_two_lines[{eps:e}]"),
            plane(v(x, 0.0, 0.0), X).as_ref(),
            cylinder(O, Z, 1.0).as_ref(),
            &lines,
        );
    }
}

#[test]
fn cylinders_whose_axes_cross_and_vivianis_pair_decline_to_the_pencil_method() {
    let c = cylinder(O, Z, 1.0);
    let crossing = [
        ("cc_steinmetz", cylinder(O, X, 1.0)),
        ("cc_perp_unequal", cylinder(O, X, 0.6)),
        ("cc_skew_two_loops", cylinder(v(0.0, 0.2, 0.0), X, 0.6)),
        ("cc_skew_one_loop", cylinder(v(0.0, 0.6, 0.0), X, 0.6)),
        ("cc_figure_eight", cylinder(v(0.0, 0.4, 0.0), X, 0.6)),
        ("cc_oblique", cylinder(v(0.0, 0.1, 0.0), v(1.0, 0.0, 2.0), 0.5)),
    ];
    for (name, other) in &crossing {
        declines(name, c.as_ref(), other.as_ref());
    }
    declines("sc_viviani", cylinder(X, Z, 1.0).as_ref(), sphere(O, 2.0).as_ref());
}
