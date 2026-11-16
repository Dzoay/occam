//! Source: original
//! The pencil method against P5's quadric pairs, a seeded generated set under rigid placements with every degenerate
//! class built in by construction, and what it must not do: guess where the band cannot decide, fit a degenerate
//! pencil, or answer a swapped pair otherwise than with its images swapped. Every curve is checked as the closed
//! forms' are: each sampled point within 1e-12 of the part's size of both surfaces, each image mapping onto it.

mod classes;
mod degenerate;
mod generated;
mod p5;
mod touch;

use std::sync::Arc;

use occam_geom::{Sphere, Surface};
use occam_units::{Direction, Placement, Vec3, World};

use super::{Class, Pair, Pencil, classify};
use crate::closed::tests::{SIZE, Want, contact_maps, frame_by, from_curve, lies_on_both, mm, nearer, tol};
use crate::decide::Decisions;
use crate::{Budget, Closed, Contact, Methods, Section};

/// The closed forms, then the pencil method, as `booleans` asks them.
fn both(a: &dyn Surface, b: &dyn Surface) -> Section {
    Methods::new(vec![Box::new(Closed), Box::new(Pencil)]).section(a, b, tol(), Budget { steps: 16 })
}

/// The pencil method alone.
fn alone(a: &dyn Surface, b: &dyn Surface) -> Section {
    Methods::new(vec![Box::new(Pencil)]).section(a, b, tol(), Budget { steps: 16 })
}

/// The class the pencil of `a` and `b` is found to be.
fn class(a: &dyn Surface, b: &dyn Surface) -> Class {
    let pair = Pair::new(a.analytic(tol()).unwrap(), b.analytic(tol()).unwrap()).unwrap();
    classify(&pair, &mut Decisions::new(tol())).unwrap()
}

/// A case: its name, its two surfaces, what it gives in its own frame, and the placement that takes that to the world.
pub(super) struct Case {
    name: String,
    a: Arc<dyn Surface>,
    b: Arc<dyn Surface>,
    want: Want,
    place: Placement<World, World>,
    /// Contacts it has beyond those `want` names, where they are not known in advance.
    unnamed: usize,
}

/// Where `place` takes `p`.
fn moved(place: &Placement<World, World>, p: Vec3) -> Vec3 {
    place.point(occam_units::Point::from_vec3(p)).to_vec3()
}

/// The pencil method answers the case as it says, with every point on both surfaces and every image on its curve.
fn check(case: &Case) {
    let (name, a, b, want) = (&case.name, case.a.as_ref(), case.b.as_ref(), &case.want);
    let got = both(a, b);
    assert_eq!(got.method, Some("pencil"), "{name}: {:?}", got.failures);
    assert!(got.failures.is_empty(), "{name}: {:?}", got.failures);
    let contacts = want.contacts.len().saturating_add(case.unnamed);
    assert_eq!((got.curves.len(), got.contacts.len()), (want.curves, contacts), "{name}");
    for p in want.on.iter().map(|p| moved(&case.place, *p)) {
        let near = got.curves.iter().map(|c| from_curve(c.curve.as_ref(), p)).fold(f64::INFINITY, nearer);
        assert!(near <= 1e-9 * SIZE, "{name}: no curve passes through {p:?}: the nearest is {near} away");
    }
    for p in want.contacts.iter().map(|p| moved(&case.place, *p)) {
        let near = |c: &Contact| matches!(c, Contact::Point { at, .. } if (at.to_vec3() - p).norm() <= 1e-10 * SIZE);
        assert!(got.contacts.iter().any(near), "{name}: no contact at {p:?} in {:?}", got.contacts);
    }
    for found in &got.contacts {
        contact_maps(name, found, a, b);
    }
    for piece in &got.curves {
        lies_on_both(name, piece, a, b);
    }
}

/// A cylinder about the axis through `at` along `axis`, placed by `place`.
fn cylinder_by(at: Vec3, axis: Vec3, radius: f64, place: &Placement<World, World>) -> Arc<dyn Surface> {
    Arc::new(occam_geom::Cylinder::placed(&frame_by(at, axis, place), mm(radius)).unwrap())
}

/// The cone of both nappes with its apex at `apex`, opening along `axis` by `half`, placed by `place`.
fn cone_by(apex: Vec3, axis: Vec3, half: f64, place: &Placement<World, World>) -> Arc<dyn Surface> {
    let angle = occam_units::Angle::from_radians(half);
    Arc::new(occam_geom::Cone::placed(&frame_by(apex, axis, place), mm(0.0), angle).unwrap())
}

/// A sphere about `centre` whose own axis, and so its poles, is along `axis`.
fn sphere_along(centre: Vec3, axis: Vec3, radius: f64, place: &Placement<World, World>) -> Arc<dyn Surface> {
    Arc::new(Sphere::placed(&frame_by(centre, axis, place), mm(radius)).unwrap())
}

/// A seeded sequence of numbers in `[0, 1)`: the fractional parts of a start plus multiples of the golden ratio.
pub(super) struct Seeded(f64);

impl Seeded {
    pub(super) fn new(seed: u32) -> Self {
        Self(0.5 * f64::from(seed) / (1.0 + f64::from(seed)))
    }

    /// The next number, from `lo` to `hi`.
    pub(super) fn next(&mut self, lo: f64, hi: f64) -> f64 {
        self.0 = (self.0 + 0.618_033_988_749_894_9).fract();
        lo + (hi - lo) * self.0
    }

    /// A rigid placement: a turn about an axis it picks by an angle it picks, and a shift.
    pub(super) fn rigid(&mut self) -> Placement<World, World> {
        let axis = Vec3::new(self.next(-1.0, 1.0), self.next(-1.0, 1.0), self.next(0.2, 1.0));
        let turn =
            Placement::rotation(Direction::normalised(axis), occam_units::Angle::from_radians(self.next(0.3, 3.0)));
        let by = Vec3::new(self.next(-3.0, 3.0), self.next(-3.0, 3.0), self.next(-3.0, 3.0));
        turn.then(&Placement::translation(occam_units::Vector::from_vec3(by)))
    }
}
