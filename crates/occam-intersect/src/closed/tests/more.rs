//! Source: original
//! The closed forms beyond P5's cases: the rounding margin that P5's hull test lacked, a B-spline that is really a
//! plane (S4), coaxial surfaces of revolution with a torus among them, a circle through a sphere's poles, the pairs
//! left to other methods, and a decision the tolerance cannot make, which is reported and not guessed.

use std::f64::consts::FRAC_PI_6;
use std::sync::Arc;

use occam_geom::{BsplineSurface, Plane, Sphere, SplineSurface, Surface};
use occam_report::{Code, Fault, NamedValue};
use occam_units::{Angle, Length, Placement, Point, Tolerance, Vec3, Vector, World};

use super::{
    Coincidence, O, Want, X, Z, check, circle, coincident, cone, curves, cylinder, declines, frame_by, lies_on_both,
    mm, plane, rigid, section, section_at, sphere, tol, torus, v,
};

#[test]
fn a_plane_exactly_tangent_to_a_sphere_gives_a_contact_under_every_placement() {
    // P5's hull test rejected this pair for want of a rounding margin. Each placement rounds the two surfaces' numbers
    // differently, and every one is a contact.
    for i in 0..24_u32 {
        let k = f64::from(i);
        let place = super::rigid_by(v(1.0 + k, 2.0 - k, 0.5 * k + 0.1), 0.37 * k + 0.1, v(k, -2.0 * k, 0.25 * k));
        let normal = v(0.3 * k - 2.0, 1.0, 0.7);
        let unit = normal * (1.0 / normal.norm());
        let centre = v(0.5, -0.25, 1.5);
        let s: Arc<dyn Surface> = Arc::new(Sphere::placed(&frame_by(centre, Z, &place), mm(1.3)).unwrap());
        let p: Arc<dyn Surface> = Arc::new(Plane::placed(&frame_by(centre + unit * 1.3, unit, &place)));
        let got = section(p.as_ref(), s.as_ref());
        assert!(got.failures.is_empty(), "{i}: {:?}", got.failures);
        assert_eq!((got.curves.len(), got.contacts.len()), (0, 1), "{i}");
    }
}

/// A bilinear patch on the square of side 6 about `centre` in the plane facing `normal`, placed, its parameters
/// running from 0 to 1 along each side.
fn patch(centre: Vec3, normal: Vec3) -> Arc<dyn Surface> {
    let placed = frame_by(centre, normal, &rigid());
    let corner = |x: f64, y: f64| placed.point(Point::from_vec3(v(x, y, 0.0)));
    let poles = vec![corner(-3.0, -3.0), corner(3.0, -3.0), corner(-3.0, 3.0), corner(3.0, 3.0)];
    let knots = vec![0.0, 0.0, 1.0, 1.0];
    let form =
        BsplineSurface { degree: (1, 1), count: (2, 2), knots_u: knots.clone(), knots_v: knots, poles, weights: None };
    Arc::new(SplineSurface::new(&form).unwrap())
}

#[test]
fn a_bspline_patch_that_is_really_a_plane_takes_the_closed_form() {
    let (flat, s) = (patch(v(0.0, 0.0, 0.3), Z), sphere(O, 1.0));
    check("bspline_sphere", flat.as_ref(), s.as_ref(), &curves(1, circle(v(0.0, 0.0, 0.3), Z, 0.91_f64.sqrt(), 5)));
    // A patch is bounded, so the pair is one whose section lies inside it: an ellipse of half-axes 0.4 and 0.2 √5.
    let tilted = patch(O, v(1.0, 0.0, 2.0));
    check("bspline_cylinder", tilted.as_ref(), cylinder(O, Z, 0.4).as_ref(), &curves(1, vec![v(0.0, 0.4, 0.0)]));
}

#[test]
fn a_plane_whose_parameters_are_not_affine_declines_since_its_images_have_no_closed_form() {
    // Poles in the plane z = 0.3, bunched towards one side, so that the patch's point moves unevenly with `u`.
    let at = |x: f64, y: f64| Point::<World>::from_vec3(super::placed(v(x, y, 0.3)));
    let poles = vec![at(-2.0, -2.0), at(-1.5, -2.0), at(2.0, -2.0), at(-2.0, 2.0), at(-1.5, 2.0), at(2.0, 2.0)];
    let form = BsplineSurface {
        degree: (2, 1),
        count: (3, 2),
        knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 1.0, 1.0],
        poles,
        weights: None,
    };
    let bunched: Arc<dyn Surface> = Arc::new(SplineSurface::new(&form).unwrap());
    assert!(bunched.analytic(tol()).is_some(), "it presents its plane");
    declines("bunched_sphere", bunched.as_ref(), sphere(O, 1.0).as_ref());
}

#[test]
fn a_cone_with_a_cylinder_off_its_axis_declines() {
    let c = cone(O, Z, FRAC_PI_6);
    declines("cone_cyl_crossing", c.as_ref(), cylinder(O, X, 0.5).as_ref());
    declines("cone_cyl_parallel", c.as_ref(), cylinder(v(0.3, 0.0, 0.0), Z, 0.5).as_ref());
    declines("sphere_cyl_off_axis", sphere(v(0.2, 0.0, 0.0), 1.0).as_ref(), cylinder(O, Z, 0.5).as_ref());
    declines("cone_sphere_off_axis", c.as_ref(), sphere(v(0.0, 0.5, 1.0), 0.5).as_ref());
}

#[test]
fn coaxial_surfaces_of_revolution_meet_in_circles_where_their_meridians_cross_a_torus_included() {
    let t = torus(O, Z, 2.0, 0.5);
    let ring = |z: f64, r: f64| circle(v(0.0, 0.0, z), Z, r, 3);
    let across = (0.25_f64 - 0.04).sqrt();
    let (rho, zeta) = (1.9375_f64, (4.0_f64 - 1.9375 * 1.9375).sqrt());
    let cases = [
        ("torus_cylinder", cylinder(v(0.0, 0.0, 3.0), -Z, 2.0), [ring(0.5, 2.0), ring(-0.5, 2.0)].concat(), 2),
        ("torus_plane", plane(v(0.0, 0.0, 0.2), Z), [ring(0.2, 2.0 + across), ring(0.2, 2.0 - across)].concat(), 2),
        ("torus_plane_tangent", plane(v(0.0, 0.0, 0.5), -Z), ring(0.5, 2.0), 1),
        ("torus_sphere", sphere(O, 2.0), [ring(zeta, rho), ring(-zeta, rho)].concat(), 2),
        ("torus_cone", cone(v(0.0, 0.0, -3.0), Z, FRAC_PI_6), vec![], 2),
    ];
    for (name, other, on, count) in cases {
        check(name, t.as_ref(), other.as_ref(), &curves(count, on));
    }
    check("torus_torus", t.as_ref(), torus(O, -Z, 2.0, 0.5).as_ref(), &coincident(Coincidence::Coaxial));
    let reversed = cone(O, -Z, FRAC_PI_6);
    check("cone_cone", cone(O, Z, FRAC_PI_6).as_ref(), reversed.as_ref(), &coincident(Coincidence::Coaxial));
    let upright = cone(O, Z, FRAC_PI_6);
    check("cone_sphere", upright.as_ref(), sphere(v(0.0, 0.0, 2.0), 1.2).as_ref(), &curves(2, vec![]));
    let inscribed = ring(1.5, 0.75_f64.sqrt());
    check("cone_sphere_tangent", upright.as_ref(), sphere(v(0.0, 0.0, 2.0), 1.0).as_ref(), &curves(1, inscribed));
    let apex_on = Want { curves: 1, contacts: vec![Z], on: ring(-0.5, 0.75_f64.sqrt()), ..Want::default() };
    check("cone_apex_sphere", cone(Z, Z, FRAC_PI_6).as_ref(), sphere(O, 1.0).as_ref(), &apex_on);
    declines("torus_plane_through_axis", t.as_ref(), plane(O, X).as_ref());
}

#[test]
fn a_circle_through_a_spheres_poles_is_cut_there_so_that_its_image_on_the_sphere_does_not_jump() {
    let s = sphere(O, 1.0);
    let tilted = v(1.0, 0.0, 1.0) * (1.0 / 2.0_f64.sqrt());
    let cases = [("through_both", plane(O, X), 2), ("through_one", plane(Z, tilted), 1)];
    for (name, p, arcs) in cases {
        let got = section(p.as_ref(), s.as_ref());
        assert!(got.failures.is_empty(), "{name}: {:?}", got.failures);
        assert_eq!(got.curves.len(), arcs, "{name}");
        for piece in &got.curves {
            lies_on_both(name, piece, p.as_ref(), s.as_ref());
            let range = piece.curve.range();
            let steps: Vec<f64> = (1..200).map(|i| range.lo + (range.hi - range.lo) * f64::from(i) / 200.0).collect();
            for pair in steps.windows(2) {
                let (a, b) = (piece.on_b.uv(pair[0]), piece.on_b.uv(pair[1]));
                let jump = (a.u() - b.u()).abs() + (a.v() - b.v()).abs();
                assert!(jump < 0.5, "{name}: the image on the sphere jumps by {jump} at {}", pair[0]);
            }
        }
    }
}

#[test]
fn swapping_a_pair_swaps_the_images_and_nothing_else() {
    let (p, c) = (plane(Z, v(1.0, 0.0, 2.0)), cone(O, Z, FRAC_PI_6));
    let (one, other) = (section(p.as_ref(), c.as_ref()), section(c.as_ref(), p.as_ref()));
    assert_eq!((one.curves.len(), other.curves.len()), (1, 1), "an ellipse");
    for (x, y) in one.curves.iter().zip(&other.curves) {
        for t in [-1.0, 0.0, 2.5] {
            assert_eq!(x.curve.point(t), y.curve.point(t));
            assert_eq!((x.on_a.uv(t), x.on_b.uv(t)), (y.on_b.uv(t), y.on_a.uv(t)));
        }
    }
}

/// A plane facing up through `(0, 0, 1 + band + lift)`, over the unit sphere about the origin, neither placed, so that
/// their gap less the radius is the band exactly before rounding.
fn over_the_band(band: f64, lift: f64) -> (Arc<dyn Surface>, Arc<dyn Surface>) {
    let up = Placement::translation(Vector::from_vec3(v(0.0, 0.0, 1.0 + band + lift)));
    let p: Arc<dyn Surface> = Arc::new(Plane::placed(&Placement::identity().then(&up)));
    (p, Arc::new(Sphere::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap()))
}

/// The tolerance whose band is `band`, which `1 + band` holds exactly.
fn at_band(band: f64) -> Tolerance {
    Tolerance::new(Length::from_millimetres(band), Angle::from_radians(1e-12)).unwrap()
}

/// 2^-40.
const BAND: f64 = 1.0 / 1_099_511_627_776.0;

#[test]
fn a_gap_on_the_edge_of_the_band_is_reported_undecided_and_nothing_is_guessed() {
    let (p, s) = over_the_band(BAND, 0.0);
    let got = section_at(p.as_ref(), s.as_ref(), at_band(BAND));
    assert_eq!((got.method, got.curves.len(), got.contacts.len()), (Some("closed"), 0, 0));
    let [report] = got.failures.as_slice() else { panic!("{:?}", got.failures) };
    assert_eq!((report.code, report.fault), (Code::Undecided, Some(Fault::NumericallySensitive)));
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    let radius = value("radius").unwrap_or(0.0);
    let gap = value("quantity").map(f64::abs);
    assert_eq!((gap, value("band")), (Some(BAND), Some(BAND)), "the gap less the radius, signed, and the band");
    assert!(radius > 0.0 && radius < 1e-3 * BAND, "a rounding's radius: {radius}");
    assert!(report.remedies.is_empty(), "no remedy: {:?}", report.remedies);
    assert!(got.tally.undecided >= 1, "{:?}", got.tally);
}

#[test]
fn the_same_gap_clear_of_the_bands_edge_is_decided() {
    // Moved apart by a hundredth of the band it is clearly apart, and at twice the band clearly a tangency.
    let (p, s) = over_the_band(BAND, 0.01 * BAND);
    let apart = section_at(p.as_ref(), s.as_ref(), at_band(BAND));
    assert!(apart.failures.is_empty() && apart.contacts.is_empty() && apart.curves.is_empty(), "{:?}", apart.failures);
    let (p, s) = over_the_band(BAND, 0.0);
    let touching = section_at(p.as_ref(), s.as_ref(), at_band(2.0 * BAND));
    assert!(touching.failures.is_empty(), "{:?}", touching.failures);
    assert_eq!((touching.curves.len(), touching.contacts.len()), (0, 1));
}
