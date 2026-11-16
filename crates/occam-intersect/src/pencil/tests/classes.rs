//! Source: original
//! The classes only the pencil's determinant tells apart: a split into two conics whose crossings are not real, which
//! no touch on the rulings shows, and a cusp, a triple root, where a sphere through a cone's apex is tangent there to a
//! plane that touches the cone along a ruling. Then a cross-check on P5's pairs: where the pencil has a multiple root
//! of rank three or two with real crossings, the touches found on the rulings agree.

use occam_geom::Curve;
use occam_math::{atan2, sin_cos};

use super::touch::touches;
use super::{Case, Class, alone, check, class, sphere_along};
use crate::closed::tests::{O, Want, X, Z, cone, cylinder, from_curve, lies_on_both, rigid, sphere, tol, v};
use crate::decide::Decisions;

/// A cone and a cylinder that both circumscribe the unit sphere about the origin, so that they meet in two conics:
/// the cylinder of radius 1 about Z, and the cone from an apex 4 from the origin and 0.15 from Z, opening towards the
/// origin by the half-angle whose sine is 1/4. Their circles of tangency on the sphere do not meet, so the conics do not
/// cross where any point is real, and the two are not about one axis, so the closed forms leave them.
pub(super) fn circumscribing() -> Case {
    let (sin, cos) = sin_cos(0.15);
    let apex = v(4.0 * sin, 0.0, 4.0 * cos);
    let half = atan2(0.25, (1.0_f64 - 0.0625).sqrt());
    let want = Want { curves: 2, ..Want::default() };
    Case {
        name: "circumscribing".to_string(),
        a: cylinder(O, Z, 1.0),
        b: cone(apex, -apex, half),
        want,
        place: rigid(),
        unnamed: 0,
    }
}

#[test]
fn a_split_whose_conics_cross_at_no_real_point_is_classified_as_a_split() {
    let case = circumscribing();
    assert!(
        matches!(class(case.a.as_ref(), case.b.as_ref()), Class::Split { .. }),
        "{:?}",
        class(case.a.as_ref(), case.b.as_ref())
    );
    check(&case);
    let coaxial = [
        ("sc_two_circles", sphere(O, 1.0), cylinder(O, Z, 0.6)),
        ("sphere_round_x", sphere(O, 1.0), cylinder(O, X, 0.8)),
    ];
    for (name, a, b) in coaxial {
        assert!(matches!(class(a.as_ref(), b.as_ref()), Class::Split { .. }), "{name}");
        let got = alone(a.as_ref(), b.as_ref());
        assert!(got.failures.is_empty(), "{name}: {:?}", got.failures);
        assert_eq!((got.curves.len(), got.contacts.len()), (2, 0), "{name}: two circles that do not cross");
        for piece in &got.curves {
            lies_on_both(name, piece, a.as_ref(), b.as_ref());
        }
    }
}

/// The cone of half-angle 0.4 about Z with its apex at the origin, and the sphere of radius 1.5 through the apex whose
/// centre lies along the cone's outward normal on the ruling in the XZ plane, so that its tangent plane at the apex
/// touches the cone along that ruling. Its curve is one loop from the cusp back to it, the rulings' parameter starting
/// where the curve passes the apex.
pub(super) fn cusp() -> Case {
    let (sa, ca) = sin_cos(0.4);
    let centre = v(ca, 0.0, -sa) * 1.5;
    let on = (0..7)
        .map(|k| {
            let (s, c) = sin_cos(0.4 + 0.8 * f64::from(k));
            let g = v(sa * c, sa * s, ca);
            g * (2.0 * g.dot(centre))
        })
        .collect();
    let want = Want { curves: 1, contacts: vec![O], on, ..Want::default() };
    let b = sphere_along(centre, v(0.3, 0.8, 0.5), 1.5, &rigid());
    Case { name: "cusp".to_string(), a: cone(O, Z, 0.4), b, want, place: rigid(), unnamed: 0 }
}

#[test]
fn a_cusp_is_a_triple_root_parametrized_rationally_through_it() {
    let case = cusp();
    let got = class(case.a.as_ref(), case.b.as_ref());
    assert!(matches!(got, Class::Node { kind: super::super::Singular::Cusp, .. }), "{got:?}");
    check(&case);
}

/// The touches on the rulings of `case`'s pair: `None` where the pair has no ruled member.
fn touched(case: &Case) -> Option<usize> {
    let tol = crate::closed::tests::tol();
    let pair = super::super::Pair::new(case.a.analytic(tol).unwrap(), case.b.analytic(tol).unwrap())?;
    let (r, o, ruling) = pair.ruled()?;
    let along = ruling.along(&pair.quadric(o));
    touches(&pair, (r, o, &ruling), &along, &mut Decisions::new(tol)).unwrap().map(|t| t.len())
}

#[test]
fn the_touches_on_the_rulings_agree_with_the_multiple_roots_where_those_are_real() {
    for case in &super::p5::cases() {
        let want = match class(case.a.as_ref(), case.b.as_ref()) {
            Class::Smooth => 0,
            Class::Node { .. } => 1,
            Class::Split { .. } => 2,
            other => panic!("{}: {other:?}", case.name),
        };
        assert_eq!(touched(case), Some(want), "{}", case.name);
    }
    assert_eq!(touched(&circumscribing()), Some(0), "a split whose crossings are not real shows no touch");
}

#[test]
fn a_ruling_and_a_cubic_and_a_tangency_along_a_conic_are_classified_and_reported() {
    let (sa, ca) = sin_cos(0.4);
    let (g, across) = (v(sa, 0.0, ca), v(0.0, 1.0, 0.0));
    // A cylinder whose ruling through the cone's apex is the cone's ruling, but turned off its tangent plane.
    let tube = cylinder(across * 0.5, g, 0.5);
    assert_eq!(class(tube.as_ref(), cone(O, Z, 0.4).as_ref()), Class::Other("a ruling and a cubic"));
    let inscribed = (sphere(O, 1.0), cylinder(O, Z, 1.0));
    assert_eq!(class(inscribed.0.as_ref(), inscribed.1.as_ref()), Class::Other("surfaces tangent along a conic"));
}

#[test]
fn conjugate_planes_give_no_curve_and_only_the_real_points_of_their_line() {
    let (a, b) = (sphere(O, 0.5), cylinder(O, Z, 1.0));
    assert!(
        matches!(class(a.as_ref(), b.as_ref()), Class::Conjugate { line: None }),
        "{:?}",
        class(a.as_ref(), b.as_ref())
    );
    let got = alone(a.as_ref(), b.as_ref());
    assert_eq!((got.method, got.curves.len(), got.contacts.len(), got.failures.len()), (Some("pencil"), 0, 0, 0));
}

#[test]
fn parallel_cylinders_are_left_to_the_closed_forms() {
    let got = alone(cylinder(O, Z, 1.0).as_ref(), cylinder(v(0.5, 0.0, 0.0), Z, 0.7).as_ref());
    assert_eq!((got.method, got.curves.len()), (None, 0), "{:?}", got.failures);
}

#[test]
fn the_pencils_planes_are_cut_with_a_cylinder_before_a_sphere_before_a_cone() {
    let tol = crate::closed::tests::tol();
    let cut = |a: &dyn occam_geom::Surface, b: &dyn occam_geom::Surface| {
        super::super::cut(&super::super::Pair::new(a.analytic(tol).unwrap(), b.analytic(tol).unwrap()).unwrap())
    };
    let (s, c, k) = (sphere(O, 1.0), cylinder(X, Z, 0.5), cone(Z, X, 0.4));
    assert_eq!(cut(s.as_ref(), c.as_ref()), (1, 0));
    assert_eq!(cut(c.as_ref(), s.as_ref()), (0, 1));
    assert_eq!(cut(k.as_ref(), s.as_ref()), (1, 0));
    assert_eq!(cut(s.as_ref(), k.as_ref()), (0, 1));
    assert_eq!(cut(c.as_ref(), cylinder(O, X, 0.5).as_ref()), (0, 1), "of two alike, the first");
}

/// The unit sphere and the unit cylinder about the line through `(2, 0, 0)` along Z touch at `(1, 0, 0)`, curving away
/// from each other by 2 across Z and 1 along it: a tangency below the resolution, of `κ = 1`.
#[test]
fn an_isolated_touch_is_a_contact_and_no_curve() {
    let (a, b) = (sphere(O, 1.0), cylinder(v(2.0, 0.0, 0.0), Z, 1.0));
    assert!(matches!(class(a.as_ref(), b.as_ref()), Class::Node { kind: super::super::Singular::Isolated, .. }));
    let want = Want { contacts: vec![X], ..Want::default() }.below(1.0);
    check(&Case { name: "isolated".to_string(), a, b, want, place: rigid(), unnamed: 0 });
}

/// The pair's section, counted, after the closed forms and the pencil.
fn counted(a: &dyn occam_geom::Surface, b: &dyn occam_geom::Surface) -> (usize, usize, usize) {
    let got = super::both(a, b);
    (got.curves.len(), got.contacts.len(), got.failures.len())
}

#[test]
fn a_node_at_a_cones_apex_is_cut_there_and_either_cone_of_two_may_hold_it() {
    // The sphere through the apex whose tangent plane there holds two of the cone's rulings: a crossing at the apex,
    // the curve passing it twice, once on each nappe.
    let crossing = Case {
        name: "apex_crossing".to_string(),
        a: cone(O, Z, 0.4),
        b: sphere(X * 1.5, 1.5),
        want: Want { curves: 2, contacts: vec![O], ..Want::default() },
        place: rigid(),
        unnamed: 0,
    };
    assert!(matches!(
        class(crossing.a.as_ref(), crossing.b.as_ref()),
        Class::Node { kind: super::super::Singular::Crossing, .. }
    ));
    check(&crossing);
    // A narrow cone whose ruling along Z runs through the apex of a wide one about Z, sharing no direction with it.
    let (sb, cb) = sin_cos(0.15);
    let (wide, narrow) = (cone(O, Z, 0.5), cone(v(0.0, 0.0, -2.0), v(sb, 0.0, cb), 0.15));
    for (one, two) in [(&wide, &narrow), (&narrow, &wide)] {
        let got = class(one.as_ref(), two.as_ref());
        assert!(matches!(got, Class::Node { .. }), "{got:?}");
        assert_eq!(counted(one.as_ref(), two.as_ref()), (2, 1, 0), "two arcs that meet at the apex");
        let section = alone(one.as_ref(), two.as_ref());
        assert!(section.failures.is_empty(), "{:?}", section.failures);
        for piece in &section.curves {
            lies_on_both("cones", piece, one.as_ref(), two.as_ref());
        }
    }
}

#[test]
fn a_nodal_quartic_read_a_half_turn_on_is_the_same_member_and_the_same_curve() {
    let crossing = (cone(O, Z, 0.4), sphere(X * 1.5, 1.5));
    let isolated = (sphere(O, 1.0), cylinder(v(2.0, 0.0, 0.0), Z, 1.0));
    // A narrow cone with its apex on a wide one and its axis along the wide one's: its rulings run inside the wide one's
    // asymptotic cone, where the wide one's quadric is negative, and so is every weight of its nodal quartic.
    let (s, c) = sin_cos(0.6);
    let inside = (cone(O, Z, 0.6), cone(v(2.0 * s, 0.0, 2.0 * c), Z, 0.2));
    for (a, b) in [crossing, isolated, (cusp().a, cusp().b), inside] {
        let pair = super::super::Pair::new(a.analytic(tol()).unwrap(), b.analytic(tol()).unwrap()).unwrap();
        let Ok(Class::Node { phi, at, .. }) = super::super::classify(&pair, &mut Decisions::new(tol())) else {
            panic!("not a node")
        };
        let [one, other] = [phi, phi + std::f64::consts::PI].map(|turn| super::super::nodal::node(&pair, at, turn));
        let (Ok(one), Ok(other)) = (one, other) else { panic!("a node read a half turn on is refused") };
        let (Some(one), Some(other)) = (one.curve, other.curve) else { continue };
        let range = Curve::range(&one);
        for i in 0..=32 {
            let p = one.point(range.lo + (range.hi - range.lo) * f64::from(i) / 32.0).to_vec3();
            assert!(from_curve(&other, p) <= 1e-9, "{p:?} is off the curve read a half turn on");
        }
    }
}
