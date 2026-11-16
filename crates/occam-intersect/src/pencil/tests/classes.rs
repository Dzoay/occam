//! Source: original
//! The classes only the pencil's determinant tells apart: a split into two conics whose crossings are not real, which
//! no touch on the rulings shows, and a cusp, a triple root, where a sphere through a cone's apex is tangent there to a
//! plane that touches the cone along a ruling. Then a cross-check on P5's pairs: where the pencil has a multiple root
//! of rank three or two with real crossings, the touches found on the rulings agree.

use occam_math::{atan2, sin_cos};

use super::touch::touches;
use super::{Case, Class, alone, check, class, sphere_along};
use crate::closed::tests::{O, Want, X, Z, cone, cylinder, lies_on_both, rigid, sphere, v};
use crate::decide::Decisions;

/// A cone and a cylinder that both circumscribe the unit sphere about the origin, so that they meet in two conics:
/// the cylinder of radius 1 about Z, and the cone from an apex 4 from the origin and 0.15 from Z, opening towards the
/// origin by the half-angle whose sine is 1/4. Their circles of tangency on the sphere do not meet, so the conics do not
/// cross where any point is real, and the two are not about one axis, so the closed forms leave them.
fn circumscribing() -> Case {
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
fn cusp() -> Case {
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
