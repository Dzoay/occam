//! Source: original
//! P5's quadric pairs with no closed form (`docs/evidence/2026-11-09-p5.md`): its six cylinders whose axes cross and
//! Viviani's sphere and cylinder, rebuilt with the answers its spike knew by construction under P5's rigid placement.
//! Along each ruling of the second cylinder the first is a quadratic, so the points of the true section are known
//! there in closed form, independently of the method.

use std::f64::consts::{PI, TAU};

use occam_math::sin_cos;
use occam_units::Vec3;

use super::{Case, Class, check, class};
use crate::closed::tests::{O, Want, X, Z, cylinder, rigid, sphere, v};

/// The points where the rulings of the cylinder of radius `rho` through `(0, e, 0)` along the unit `d`, at `n` angles,
/// meet the cylinder of radius 1 about Z, where they do. `d` lies in the XZ plane, so the cross-section's Y axis is
/// Y itself.
fn on_rulings(offset: f64, rho: f64, along: Vec3, count: u32) -> Vec<Vec3> {
    let up = v(0.0, 1.0, 0.0);
    let across = along.cross(up);
    let mut out = Vec::new();
    for i in 0..count {
        let (sine, cosine) = sin_cos(TAU * (f64::from(i) + 0.3) / f64::from(count));
        let base = v(0.0, offset, 0.0) + up * (rho * cosine) + across * (rho * sine);
        let square = along.x * along.x + along.y * along.y;
        let half = base.x * along.x + base.y * along.y;
        let rest = base.x * base.x + base.y * base.y - 1.0;
        let disc = half * half - square * rest;
        if disc > 1e-9 {
            for sign in [1.0, -1.0] {
                out.push(base + along * ((-half + sign * disc.sqrt()) / square));
            }
        }
    }
    out
}

fn case(name: &str, other: (Vec3, Vec3, f64), want: Want) -> Case {
    let (at, axis, radius) = other;
    Case {
        name: name.to_string(),
        a: cylinder(O, Z, 1.0),
        b: cylinder(at, axis, radius),
        want,
        place: rigid(),
        unnamed: 0,
    }
}

fn curves(count: usize, on: Vec<Vec3>, contacts: Vec<Vec3>) -> Want {
    Want { curves: count, contacts, on, ..Want::default() }
}

/// P5's six cylinders whose axes cross: two equal ones, unequal ones square to each other with their axes apart by
/// 0, 0.2, 0.6 and 0.4, the last tangent where the two touch, and one oblique.
fn crossing() -> Vec<Case> {
    let steinmetz: Vec<Vec3> = (0..6)
        .flat_map(|i| {
            let (sin, cos) = sin_cos(f64::from(i) + 0.2);
            [v(cos, sin, cos), v(cos, sin, -cos)]
        })
        .collect();
    let oblique = v(1.0, 0.0, 2.0) * (1.0 / 5.0_f64.sqrt());
    vec![
        case("cc_steinmetz", (O, X, 1.0), curves(4, steinmetz, vec![v(0.0, 1.0, 0.0), v(0.0, -1.0, 0.0)]).below(1.0)),
        case("cc_perp_unequal", (O, X, 0.6), curves(2, on_rulings(0.0, 0.6, X, 12), vec![])),
        case("cc_skew_two_loops", (v(0.0, 0.2, 0.0), X, 0.6), curves(2, on_rulings(0.2, 0.6, X, 12), vec![])),
        case("cc_skew_one_loop", (v(0.0, 0.6, 0.0), X, 0.6), curves(1, on_rulings(0.6, 0.6, X, 12), vec![])),
        case(
            "cc_figure_eight",
            (v(0.0, 0.4, 0.0), X, 0.6),
            curves(2, on_rulings(0.4, 0.6, X, 12), vec![v(0.0, 1.0, 0.0)]).below(1.0),
        ),
        case("cc_oblique", (v(0.0, 0.1, 0.0), oblique, 0.5), curves(2, on_rulings(0.1, 0.5, oblique, 12), vec![])),
    ]
}

/// Viviani's pair: the cylinder of radius 1 inside the sphere of radius 2, tangent to it at `(2, 0, 0)`, where the
/// curve has its node, a branch point, where it is cut. The sphere's own axis is Z, so the curve passes through both its
/// poles, where its image on the sphere is cut too: it comes as four pieces. The relative curvature at the node is
/// `±1/2`, so the node is below the resolution.
fn viviani() -> Case {
    let on = (0..9)
        .map(|i| {
            let t = 4.0 * PI * (f64::from(i) + 0.3) / 9.0;
            let (sin, cos) = sin_cos(t);
            v(1.0 + cos, sin, 2.0 * sin_cos(0.5 * t).0)
        })
        .collect();
    let want = curves(4, on, vec![v(2.0, 0.0, 0.0)]).below(0.5);
    Case { name: "sc_viviani".to_string(), a: cylinder(X, Z, 1.0), b: sphere(O, 2.0), want, place: rigid(), unnamed: 0 }
}

/// P5's seven quadric pairs.
pub(super) fn cases() -> Vec<Case> {
    crossing().into_iter().chain([viviani()]).collect()
}

#[test]
fn p5s_quadric_pairs_are_intersected_exactly_on_both_surfaces() {
    let cases: Vec<Case> = crossing().into_iter().chain([viviani()]).collect();
    assert_eq!(cases.len(), 7, "P5's six cylinders whose axes cross, and Viviani's pair");
    for case in &cases {
        check(case);
    }
}

#[test]
fn p5s_degenerate_pencils_are_classified_as_they_are_built() {
    let cases: Vec<Case> = crossing().into_iter().chain([viviani()]).collect();
    let want = |name: &str| match name {
        "cc_steinmetz" => "split",
        "cc_figure_eight" | "sc_viviani" => "node",
        _ => "smooth",
    };
    for case in &cases {
        let got = match class(case.a.as_ref(), case.b.as_ref()) {
            Class::Smooth => "smooth",
            Class::Node { .. } => "node",
            Class::Split { .. } => "split",
            other => panic!("{}: {other:?}", case.name),
        };
        assert_eq!(got, want(&case.name), "{}", case.name);
    }
}
