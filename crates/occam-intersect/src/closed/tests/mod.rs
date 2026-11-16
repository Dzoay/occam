//! Source: original
//! The closed forms against P5's cases, rebuilt with their answers known by construction and each placed by P5's rigid
//! placement, so that none is axis-aligned. Each is checked as P5 checked it: every point of every curve within 1e-12
//! of the part's size of both surfaces. Each image is checked too: it maps onto its curve, and its rate carries the
//! surface's derivatives onto the curve's.

mod edges;
mod more;
mod p5;

use std::sync::Arc;

use occam_geom::{Cone, Curve, Cylinder, ParamRange, Plane, Scratch, Sphere, Surface, Torus, Wrap};
use occam_math::atan2;
use occam_units::{Angle, Direction, Length, Local, ParamKind, Placement, Point, Tolerance, Vec3, Vector, World};

use crate::{Budget, Closed, Coincidence, Contact, Methods, Section, SectionCurve};

/// The part's size: P5's cases are about four across.
pub(crate) const SIZE: f64 = 4.0;

thread_local! {
    /// The scale this thread's cases are made at: 1, or what a test scales its surfaces by.
    static SCALE: std::cell::Cell<f64> = const { std::cell::Cell::new(1.0) };
}

/// Checks this thread's cases at `scale`, its parts' size and its band scaled with them, until the guard it gives goes.
pub(crate) fn at_scale(scale: f64) -> AtScale {
    SCALE.with(|s| s.set(scale));
    AtScale
}

/// While it lasts, this thread's cases are checked at the scale it was given.
pub(crate) struct AtScale;

impl Drop for AtScale {
    fn drop(&mut self) {
        SCALE.with(|s| s.set(1.0));
    }
}

/// The scale this thread's cases are checked at.
pub(crate) fn scale() -> f64 {
    SCALE.with(std::cell::Cell::get)
}

/// The part's size at this thread's scale.
pub(crate) fn size() -> f64 {
    SIZE * scale()
}

/// The tolerance the cases are decided at: fine enough that P5's near-tangent sweep, down to a gap of 1e-12, is told
/// apart from a tangency, as P5's closed forms told it; its band scaled with the part.
pub(crate) fn tol() -> Tolerance {
    Tolerance::new(Length::from_millimetres(1e-13 * scale()), Angle::from_radians(1e-12)).unwrap()
}

pub(crate) const fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

pub(crate) const Z: Vec3 = v(0.0, 0.0, 1.0);
pub(crate) const X: Vec3 = v(1.0, 0.0, 0.0);
pub(crate) const O: Vec3 = v(0.0, 0.0, 0.0);

/// A turn by `angle` about `axis`, then a shift by `by`.
pub(crate) fn rigid_by(axis: Vec3, angle: f64, by: Vec3) -> Placement<World, World> {
    let spin = Placement::rotation(Direction::normalised(axis), Angle::from_radians(angle));
    spin.then(&Placement::translation(Vector::from_vec3(by)))
}

thread_local! {
    /// Whether this thread's cases are built under P5's rigid placement, or as they are written, on the world's axes.
    static PLACED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Builds this thread's cases on the world's axes until the guard it gives goes.
fn unplaced() -> Unplaced {
    PLACED.with(|placed| placed.set(false));
    Unplaced
}

/// While it lasts, this thread's cases are unplaced.
struct Unplaced;

impl Drop for Unplaced {
    fn drop(&mut self) {
        PLACED.with(|placed| placed.set(true));
    }
}

/// P5's rigid placement, a turn of 0.7 about (1, 2, 3) then a shift of (0.3, -0.2, 0.5), or none where this thread's
/// cases are unplaced.
pub(crate) fn rigid() -> Placement<World, World> {
    if PLACED.with(std::cell::Cell::get) {
        rigid_by(v(1.0, 2.0, 3.0), 0.7, v(0.3, -0.2, 0.5))
    } else {
        Placement::identity()
    }
}

/// Where the rigid placement takes `p`.
pub(crate) fn placed(p: Vec3) -> Vec3 {
    rigid().point(Point::from_vec3(p)).to_vec3()
}

/// The frame at `origin` with its Z along `z`, then moved by `rigid`.
pub(crate) fn frame_by(origin: Vec3, z: Vec3, rigid: &Placement<World, World>) -> Placement<Local, World> {
    let unit = z * (1.0 / z.norm());
    let across = Z.cross(unit);
    let turn = if across.norm() > 0.0 {
        let angle = atan2(across.norm(), Z.dot(unit));
        Placement::rotation(Direction::normalised(across), Angle::from_radians(angle))
    } else if unit.z > 0.0 {
        Placement::identity()
    } else {
        Placement::rotation(Direction::normalised(X), Angle::from_radians(std::f64::consts::PI))
    };
    let shift = Placement::translation(Vector::from_vec3(origin));
    Placement::identity().then(&turn).then(&shift).then(rigid)
}

pub(crate) fn frame(origin: Vec3, z: Vec3) -> Placement<Local, World> {
    frame_by(origin, z, &rigid())
}

pub(crate) fn mm(x: f64) -> Length {
    Length::from_millimetres(x)
}

pub(crate) fn plane(origin: Vec3, normal: Vec3) -> Arc<dyn Surface> {
    Arc::new(Plane::placed(&frame(origin, normal)))
}

pub(crate) fn sphere(centre: Vec3, radius: f64) -> Arc<dyn Surface> {
    Arc::new(Sphere::placed(&frame(centre, Z), mm(radius)).unwrap())
}

pub(crate) fn cylinder(on_axis: Vec3, axis: Vec3, radius: f64) -> Arc<dyn Surface> {
    Arc::new(Cylinder::placed(&frame(on_axis, axis), mm(radius)).unwrap())
}

/// The cone with its apex at `apex`, both nappes.
pub(crate) fn cone(apex: Vec3, axis: Vec3, half_angle: f64) -> Arc<dyn Surface> {
    Arc::new(Cone::placed(&frame(apex, axis), mm(0.0), Angle::from_radians(half_angle)).unwrap())
}

pub(crate) fn torus(centre: Vec3, axis: Vec3, major: f64, minor: f64) -> Arc<dyn Surface> {
    Arc::new(Torus::placed(&frame(centre, axis), mm(major), mm(minor)).unwrap())
}

/// What a case gives, known by construction, before the rigid placement: how many curves, the contacts, any
/// coincidence, and points of the true section that some curve passes through.
#[derive(Default)]
pub(crate) struct Want {
    pub(crate) curves: usize,
    pub(crate) contacts: Vec<Vec3>,
    pub(crate) coincidence: Option<Coincidence>,
    pub(crate) on: Vec<Vec3>,
}

/// `count` curves, which between them pass through every point of `on`.
pub(crate) fn curves(count: usize, on: Vec<Vec3>) -> Want {
    Want { curves: count, on, ..Want::default() }
}

pub(crate) fn contact(at: Vec3) -> Want {
    Want { contacts: vec![at], ..Want::default() }
}

pub(crate) fn nothing() -> Want {
    Want::default()
}

pub(crate) fn coincident(kind: Coincidence) -> Want {
    Want { coincidence: Some(kind), ..Want::default() }
}

/// A case: its name, its two surfaces and what it gives.
pub(crate) type Case = (String, Arc<dyn Surface>, Arc<dyn Surface>, Want);

pub(crate) fn case(name: &str, a: Arc<dyn Surface>, b: Arc<dyn Surface>, want: Want) -> Case {
    (name.to_string(), a, b, want)
}

/// The section by the closed forms alone, at `tol`.
pub(crate) fn section_at(a: &dyn Surface, b: &dyn Surface, tol: Tolerance) -> Section {
    Methods::new(vec![Box::new(Closed)]).section(a, b, tol, Budget { steps: 16 })
}

pub(crate) fn section(a: &dyn Surface, b: &dyn Surface) -> Section {
    section_at(a, b, tol())
}

/// How far `p` is from `surface`.
pub(crate) fn off(surface: &dyn Surface, p: Vec3) -> f64 {
    surface.closest(Point::from_vec3(p), None, tol(), &mut Scratch::default()).unwrap().distance.millimetres()
}

/// How far `p` is from `curve`.
pub(crate) fn from_curve(curve: &dyn Curve, p: Vec3) -> f64 {
    let found = curve.closest(Point::from_vec3(p), None, tol(), &mut Scratch::default());
    found.map_or(f64::INFINITY, |c| c.distance.millimetres())
}

/// Parameters across `range`: a whole period where it comes round, the inside of a finite one, and a stretch about 0
/// twice the part's size, or two units of a parameter with no unit, where it has no end.
pub(crate) fn samples(range: ParamRange) -> Vec<f64> {
    let (lo, hi) = match (range.lo.is_finite(), range.kind) {
        (true, _) => (range.lo, range.hi),
        (false, ParamKind::Knot) => (-2.0, 2.0),
        (false, _) => (-2.0 * size(), 2.0 * size()),
    };
    let at = |i: u32| lo + (hi - lo) * f64::from(i) / 64.0;
    if range.wrap == Wrap::Periodic { (0..64).map(at).collect() } else { (1..64).map(at).collect() }
}

/// Every sampled point of `piece` within 1e-12 of the part's size of both surfaces, and each image mapping onto it
/// within as much, with a rate that the surface's derivatives carry onto the curve's.
pub(crate) fn lies_on_both(name: &str, piece: &SectionCurve, a: &dyn Surface, b: &dyn Surface) {
    for t in samples(piece.curve.range()) {
        let (p, rate) = piece.curve.first(t);
        for (surface, image, which) in [(a, &piece.on_a, "a"), (b, &piece.on_b, "b")] {
            let gap = off(surface, p.to_vec3());
            assert!(gap <= 1e-12 * size(), "{name}: the curve at {t} is {gap} off {which}");
            let (uv, d) = image.first(t);
            let jet = surface.first(uv);
            let miss = (jet.point.to_vec3() - p.to_vec3()).norm();
            assert!(miss <= 1e-12 * size(), "{name}: {which}'s image at {t} maps {miss} from the curve");
            let carried = jet.du.rate() * d.u() + jet.dv.rate() * d.v();
            let slip = (carried - rate.rate()).norm();
            assert!(slip <= 1e-9 * (1.0 + rate.rate().norm()), "{name}: {which}'s image's rate at {t} is {slip} off");
        }
    }
}

/// A contact's parameters on both surfaces map onto it.
pub(crate) fn contact_maps(name: &str, found: &Contact, a: &dyn Surface, b: &dyn Surface) {
    let Contact::Point { at, on_a, on_b } = *found else { panic!("{name}: {found:?} is not a point") };
    for (surface, uv) in [(a, on_a), (b, on_b)] {
        let miss = (surface.point(uv).to_vec3() - at.to_vec3()).norm();
        assert!(miss <= 1e-12 * size(), "{name}: a contact's parameters map {miss} from it");
    }
}

/// The nearer of two distances.
pub(crate) fn nearer(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The closed forms answer `a` with `b` as `want` says, with every point on both surfaces.
pub(crate) fn check(name: &str, a: &dyn Surface, b: &dyn Surface, want: &Want) {
    let got = section(a, b);
    assert_eq!(got.method, Some("closed"), "{name}: {:?}", got.failures);
    assert!(got.failures.is_empty(), "{name}: {:?}", got.failures);
    let counts = (got.curves.len(), got.contacts.len(), got.coincidence);
    assert_eq!(counts, (want.curves, want.contacts.len(), want.coincidence), "{name}");
    for p in want.on.iter().map(|p| placed(*p)) {
        let near = got.curves.iter().map(|c| from_curve(c.curve.as_ref(), p)).fold(f64::INFINITY, nearer);
        assert!(near <= 1e-9 * SIZE, "{name}: no curve passes through {p:?}: the nearest is {near} away");
    }
    for p in want.contacts.iter().map(|p| placed(*p)) {
        let near = |c: &Contact| matches!(c, Contact::Point { at, .. } if (at.to_vec3() - p).norm() <= 1e-12 * SIZE);
        assert!(got.contacts.iter().any(near), "{name}: no contact at {p:?} in {:?}", got.contacts);
    }
    for found in &got.contacts {
        contact_maps(name, found, a, b);
    }
    for piece in &got.curves {
        lies_on_both(name, piece, a, b);
    }
}

/// The closed forms decline `a` with `b`, so that no method of these answers it.
pub(crate) fn declines(name: &str, a: &dyn Surface, b: &dyn Surface) {
    let got = section(a, b);
    assert_eq!((got.method, got.curves.len(), got.contacts.len()), (None, 0, 0), "{name}");
    let reasons: Vec<_> = got.failures.iter().map(|r| r.reason).collect();
    assert_eq!(reasons, [Some(occam_report::Reason::PairNotSupported)], "{name}");
}

/// `n` points of the circle about `centre`, facing `normal`, of `radius`.
pub(crate) fn circle(centre: Vec3, normal: Vec3, radius: f64, n: u32) -> Vec<Vec3> {
    let unit = normal * (1.0 / normal.norm());
    let seed = if unit.x.abs() < 0.9 { X } else { v(0.0, 1.0, 0.0) };
    let x = unit.cross(seed) * (1.0 / unit.cross(seed).norm());
    let y = unit.cross(x);
    (0..n)
        .map(|i| {
            let (sin, cos) = occam_math::sin_cos(std::f64::consts::TAU * f64::from(i) / f64::from(n));
            centre + x * (radius * cos) + y * (radius * sin)
        })
        .collect()
}
