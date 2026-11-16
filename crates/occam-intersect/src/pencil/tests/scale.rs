//! Source: original
//! Every class at every size a part may have: P5's quadric pairs and each degenerate class built in, at radii of a
//! thousandth of a millimetre, of one, of a thousand and of a hundred thousand, each pair scaled about the world's
//! origin after its rigid placement and checked with the part's size and the band scaled with it. Then the split
//! nudged to the edge of the band, at each scale with the band scaled too.

use std::f64::consts::PI;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use occam_geom::{Cylinder, Surface};
use occam_report::Code;
use occam_units::{Affine, Angle, Direction, Length, Placement, Tolerance};

use super::classes::{circumscribing, cusp};
use super::degenerate::{equal_crossing, shared_apex, tangent_along_a_ruling, viviani};
use super::{Case, Class, Seeded, alone, check, class, extent};
use crate::closed::tests::{O, Z, at_scale, cylinder, sphere, v};
use crate::{Budget, Methods, Pencil, Section};

/// The scales a part is made at: radii of a thousandth of a millimetre to a hundred metres.
pub(super) const SCALES: [f64; 4] = [1e-3, 1.0, 1e3, 1e5];

/// `surface` scaled by `k` about the world's origin.
pub(super) fn grown(surface: &Arc<dyn Surface>, k: f64) -> Arc<dyn Surface> {
    surface.moved(&Affine::similarity(Placement::identity(), k).unwrap()).unwrap().0
}

/// `case` with both its surfaces scaled by `k` about the world's origin.
fn scaled(case: Case, k: f64) -> Case {
    Case { a: grown(&case.a, k), b: grown(&case.b, k), ..case }
}

/// What a class is, by name.
pub(super) fn kind(class: &Class) -> &'static str {
    match class {
        Class::Smooth => "smooth",
        Class::Reaching => "reaching",
        Class::Node { .. } => "node",
        Class::AtInfinity => "node at infinity",
        Class::RulingAndCubic => "ruling and cubic",
        Class::Split { .. } => "split",
        Class::Lines => "lines",
        Class::Conjugate { .. } => "conjugate",
        Class::Other(_) => "other",
        Class::Parallel => "parallel",
    }
}

/// P5's pairs and each degenerate class, with the class each is built as.
fn built() -> Vec<(Case, &'static str)> {
    let p5 = super::p5::cases().into_iter().map(|case| {
        let want = match case.name.as_str() {
            "cc_steinmetz" => "split",
            "cc_figure_eight" | "sc_viviani" => "node",
            _ => "smooth",
        };
        (case, want)
    });
    let mut seed = Seeded::new(11);
    let mut out: Vec<(Case, &'static str)> = p5.collect();
    out.extend([
        (equal_crossing(&mut seed, 0), "split"),
        (viviani(&mut seed, 0), "node"),
        (shared_apex(&mut seed, 0), "lines"),
        (tangent_along_a_ruling(&mut seed, 0), "split"),
        (tangent_along_a_ruling(&mut seed, 1), "split"),
        (circumscribing(), "split"),
        (cusp(), "node"),
    ]);
    out
}

/// The cases that do not come out as built at `k`, each with why.
fn failing_at(k: f64) -> Vec<String> {
    let _scale = at_scale(k);
    let mut out = Vec::new();
    for (case, want) in built() {
        let case = scaled(case, k);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let got = kind(&class(case.a.as_ref(), case.b.as_ref()));
            assert_eq!(got, want, "classified");
            check(&case);
        }));
        if let Err(why) = outcome {
            let why = why.downcast_ref::<String>().cloned().unwrap_or_default();
            out.push(format!("{}: {}", case.name, why.lines().next().unwrap_or("")));
        }
    }
    // Conjugate planes: a sphere inside a cylinder about one axis, which the pencil alone gives nothing for.
    let inside = scaled(Case { name: "conjugate".to_string(), a: sphere(O, 0.5), b: cylinder(O, Z, 1.0), ..cusp() }, k);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        assert_eq!(kind(&class(inside.a.as_ref(), inside.b.as_ref())), "conjugate", "classified");
        let got = alone(inside.a.as_ref(), inside.b.as_ref());
        assert_eq!((got.curves.len(), got.contacts.len(), got.failures.len()), (0, 0, 0));
    }));
    if outcome.is_err() {
        out.push("conjugate: not as built".to_string());
    }
    out
}

#[test]
fn every_class_comes_out_as_built_at_every_scale_a_part_may_have() {
    let failing: Vec<(f64, Vec<String>)> = SCALES.iter().map(|k| (*k, failing_at(*k))).collect();
    for (k, cases) in &failing {
        eprintln!("at {k}: {} of {} cases fail {cases:#?}", cases.len(), built().len() + 1);
    }
    assert!(failing.iter().all(|(_, cases)| cases.is_empty()), "{failing:#?}");
}

/// The cylinders of radius `k` about Z and of radius `k (1 + 2⁻⁴⁰)` about X, neither placed, decided at `band` times
/// their gap where they come nearest.
fn nudged(k: f64, band: f64) -> Section {
    let across = Placement::rotation(Direction::normalised(v(0.0, 1.0, 0.0)), Angle::from_radians(0.5 * PI));
    let mm = Length::from_millimetres;
    let (small, large) = (k, k * (1.0 + NUDGE));
    let a: Arc<dyn Surface> = Arc::new(Cylinder::placed(&Placement::identity(), mm(small)).unwrap());
    let b: Arc<dyn Surface> = Arc::new(Cylinder::placed(&Placement::identity().then(&across), mm(large)).unwrap());
    let tol = Tolerance::new(mm(band * (large - small)), Angle::from_radians(1e-12)).unwrap();
    Methods::new(vec![Box::new(Pencil::within(extent()))]).section(a.as_ref(), b.as_ref(), tol, Budget { steps: 4 })
}

/// 2⁻⁴⁰.
const NUDGE: f64 = 1.0 / 1_099_511_627_776.0;

#[test]
fn a_split_nudged_to_the_edge_of_the_band_is_undecided_at_every_scale() {
    for k in SCALES {
        let edge = nudged(k, 1.0);
        let codes: Vec<Code> = edge.failures.iter().map(|r| r.code).collect();
        assert_eq!((edge.curves.len(), edge.contacts.len(), codes), (0, 0, vec![Code::Undecided]), "at {k}");
        let split = nudged(k, 2.0);
        assert!(split.failures.is_empty(), "at {k}: {:?}", split.failures);
        assert_eq!((split.curves.len(), split.contacts.len()), (2, 2), "at {k}: within the band, a split");
        let apart = nudged(k, 0.5);
        assert!(apart.failures.is_empty(), "at {k}: {:?}", apart.failures);
        assert_eq!((apart.curves.len(), apart.contacts.len()), (2, 0), "at {k}: clear of it, two loops");
    }
}
