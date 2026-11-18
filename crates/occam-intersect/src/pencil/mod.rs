//! Source: wang-goldman-tu-2003
//! Levin's pencil method, for the pairs of spheres, cylinders and cones the closed forms leave (Levin 1976, 1979),
//! with the pencil classified before it is parametrized (Dupont, Lazard, Lazard and Petitjean 2008; Wang, Goldman and
//! Tu 2003). The two quadrics span a pencil, and a ruled member of it, a root of its determinant, carries the curve:
//! here a cylinder of the pair, or else a cone, whose rulings the other quadric meets in a quadratic.
//!
//! The pencil is classified by the real roots of its determinant, their multiplicities and the members' ranks there
//! (`roots`), each decided against the band through occam-math's predicates on the matrices' entries, and one the band
//! cannot decide is `decide.undecided`, never a guess. Then:
//! - no multiple root: a smooth quartic, parametrized by the square root ([`occam_geom::Quartic`]);
//! - a double root of rank three, its vertex on both surfaces: a nodal quartic, a crossing, an isolated point or, a
//!   triple root, a cusp, parametrized rationally from it (`nodal`);
//! - a double root of rank two: two conics, the sections of the pencil's pair of planes, or a ruling and a conic where
//!   one plane is tangent along it (`split`);
//! - a determinant that vanishes for every member, two cones with one apex: their common rulings.
//!
//! Three classes reach infinity. Where two cones share a ruling direction, the smooth quartic, or the nodal one where
//! one's apex is on the other, runs to infinity along it. A cylinder along a ruling direction of a cone meets it in a
//! quartic with its node at infinity, rational, one point on each of the cylinder's rulings (`fan`); and where it passes
//! the cone's apex, in that ruling and a cubic, one point on each of the cone's rulings. Each is given within the
//! extent asked, in bounded pieces: the smooth quartic's loops and the rational B-splines of the others.
//!
//! Singular points are kept as contacts. A degenerate pencil of any other kind is reported, not fitted.

mod eigen;
mod emit;
mod fan;
mod nodal;
mod poly;
mod quadric;
mod roots;
mod smooth;
mod split;
#[cfg(test)]
mod tests;
mod trig;

use occam_geom::{Analytic, Chart, Surface};
use occam_math::Side;
use occam_report::{Code, Fault, NamedValue, Reason, Report};
use occam_units::{Bounds, Tolerance, Vec3};

use self::quadric::{Natural, Quadric, Ruling};
use self::roots::{Multiple, Singular};
use crate::decide::{Decisions, Undecided};
use crate::interval3::{cross3, exact3, minus, norm3};
use crate::methods::{Answer, Method, Spent, Work};
use crate::section::Section;

/// Levin's pencil method, which answers the pairs of spheres, cylinders and cones the closed forms leave. A section
/// that reaches infinity is given within an extent, a box of the world, as bounded pieces: they hold every point of it
/// in the box and reach a little past it, and a loop of it that never leaves the box is given whole. A line stays
/// whole.
pub struct Pencil {
    extent: Bounds,
}

impl Pencil {
    /// The pencil method, giving a section that reaches infinity within `extent`, such as the box of the faces a
    /// boolean meets.
    #[must_use]
    pub const fn within(extent: Bounds) -> Self {
        Self { extent }
    }
}

/// What a pencil is, as its classification finds it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    /// Four simple roots: a smooth quartic, in as many loops as it has.
    Smooth,
    /// Four simple roots, and a ruling of the ruled cone along the other's asymptotic cone: a smooth quartic that
    /// reaches infinity there.
    Reaching,
    /// A double root of rank three whose vertex is at infinity, along the axis of the pair's cylinder: a quartic with its
    /// node at infinity, one point on each of the cylinder's rulings.
    AtInfinity,
    /// A double root of rank three at the pair's cylinder, its vertex at infinity, and one at the pair's cone, its apex
    /// on the cylinder: the ruling through the apex along the cylinder's axis, and a cubic.
    RulingAndCubic,
    /// A double or triple root of rank three at `phi`, whose cone has its vertex `at`, in the world, on both surfaces.
    Node { phi: f64, at: Vec3, kind: Singular },
    /// A double root of rank two: two conics, or a ruling and a conic, each a plane's in the pair's frame.
    Split { planes: [(Vec3, Vec3); 2] },
    /// A double root of rank two whose planes are conjugate: no real curve, and only the points of their real line
    /// that are on both surfaces, where the line is finite.
    Conjugate { line: Option<[(Vec3, Vec3); 2]> },
    /// A determinant zero for every member, at the vertex two cones share: their common rulings.
    Lines,
    /// A degenerate pencil this method does not parametrize.
    Other(&'static str),
    /// Two cylinders whose axes are parallel, which the closed forms answer.
    Parallel,
}

/// The pair, in canonical order, with its quadrics in a frame about the middle of the two.
struct Pair {
    forms: [Analytic; 2],
    surfaces: [Natural; 2],
    origin: Vec3,
    quadrics: [Quadric; 2],
}

impl Pair {
    /// `a` with `b`, where both are spheres, cylinders or cones and not both spheres.
    fn new(a: Analytic, b: Analytic) -> Option<Self> {
        let surfaces = [Natural::of(a)?, Natural::of(b)?];
        if surfaces.iter().all(|s| matches!(s, Natural::Sphere { .. })) {
            return None;
        }
        let origin = (surfaces[0].at() + surfaces[1].at()) * 0.5;
        Some(Self { forms: [a, b], surfaces, origin, quadrics: surfaces.map(|s| s.quadric(origin)) })
    }

    /// A point of the pair's frame in the world.
    fn world(&self, local: Vec3) -> Vec3 {
        local + self.origin
    }

    /// The pair's length: the larger of its radii and the distance between the points its surfaces are placed by, or
    /// a millimetre where all are zero, as for two cones with one apex. The pencil's members are read in it, so that
    /// a pair a metre across is read as one a millimetre across.
    fn length(&self) -> f64 {
        let radius = |s: &Natural| match *s {
            Natural::Sphere { radius, .. } | Natural::Cylinder { radius, .. } => radius,
            Natural::Cone { .. } => 0.0,
        };
        let spread = (self.surfaces[0].at() - self.surfaces[1].at()).norm();
        let most = self.surfaces.iter().map(radius).fold(spread, |a, r| if r > a { r } else { a });
        if most > 0.0 { most } else { 1.0 }
    }

    /// The surface the curve is parametrized on, the first cylinder or else the first cone, and the other's index.
    fn ruled(&self) -> Option<(usize, usize, Ruling)> {
        let upright = self.surfaces.iter().position(|s| matches!(s, Natural::Cylinder { .. }));
        let index = upright.or_else(|| self.surfaces.iter().position(|s| matches!(s, Natural::Cone { .. })))?;
        Some((index, 1_usize.saturating_sub(index), Ruling::of(self.surfaces.get(index)?, self.origin)?))
    }

    fn form(&self, i: usize) -> Analytic {
        self.forms.get(i).copied().unwrap_or(self.forms[0])
    }

    fn quadric(&self, i: usize) -> Quadric {
        self.quadrics.get(i).copied().unwrap_or(self.quadrics[0])
    }

    fn surface(&self, i: usize) -> Natural {
        self.surfaces.get(i).copied().unwrap_or(self.surfaces[0])
    }
}

/// Whether two cones have one apex, within the band.
fn shared_apex(pair: &Pair, decide: &mut Decisions) -> Result<bool, Undecided> {
    let [Natural::Cone { apex: p, .. }, Natural::Cone { apex: q, .. }] = pair.surfaces else { return Ok(false) };
    Ok(decide.length(norm3(minus(exact3(p), exact3(q))))? == Side::Within)
}

/// Two cylinders whose axes are parallel, their vertex at infinity shared, within the angle band.
fn parallel(pair: &Pair, decide: &mut Decisions) -> Result<bool, Undecided> {
    let [Natural::Cylinder { axis: a, .. }, Natural::Cylinder { axis: b, .. }] = pair.surfaces else {
        return Ok(false);
    };
    Ok(decide.angle(norm3(cross3(exact3(a), exact3(b))))? == Side::Within)
}

/// The surface the pencil's planes are cut with, a cylinder before a sphere before a cone, and the other.
fn cut(pair: &Pair) -> (usize, usize) {
    let rank = |s: &Natural| match s {
        Natural::Cylinder { .. } => 0,
        Natural::Sphere { .. } => 1,
        Natural::Cone { .. } => 2,
    };
    if rank(&pair.surfaces[1]) < rank(&pair.surfaces[0]) { (1, 0) } else { (0, 1) }
}

/// A smooth quartic, which reaches infinity where the ruled surface is a cone some of whose rulings run along the
/// other's asymptotic cone: where `α`, the other quadric's part along them, crosses zero or touches it, each decided
/// against the angle band.
fn smooth(pair: &Pair, decide: &mut Decisions) -> Result<Class, Undecided> {
    let Some((_, o, ruling)) = pair.ruled() else { return Ok(Class::Other("no ruled member")) };
    let other = pair.quadric(o);
    let alpha = ruling.along(&other).alpha;
    Ok(if other.reached(&alpha, |theta| ruling.direction(theta), decide)? { Class::Reaching } else { Class::Smooth })
}

/// Whether the pair is a cylinder and a cone, the only pair whose node at infinity is the vertex of one of its own
/// surfaces: a sphere has no direction at infinity, two cylinders with one are parallel, and the vertex of a cylinder
/// along a direction two cones share is no surface of theirs.
///
/// Only a cylinder and a cone have a node at infinity and one vertex on both surfaces besides. The quartic is then
/// a line through both singular points and a cubic. Every line on a cone passes its apex, so for two cones the line
/// would pass both apexes, each on the other cone, two vertices, and for any other pair there is no node at infinity.
fn cylinder_and_cone(pair: &Pair) -> bool {
    let cylinder = |s: &Natural| matches!(s, Natural::Cylinder { .. });
    let cone = |s: &Natural| matches!(s, Natural::Cone { .. });
    let [a, b] = &pair.surfaces;
    (cylinder(a) && cone(b)) || (cone(a) && cylinder(b))
}

/// The class of the pencil of `pair`, from the multiple roots of its determinant.
fn classify(pair: &Pair, decide: &mut Decisions) -> Result<Class, Undecided> {
    if shared_apex(pair, decide)? {
        return Ok(Class::Lines);
    }
    if parallel(pair, decide)? {
        return Ok(Class::Parallel);
    }
    if roots::every_member_singular(pair, decide)? {
        return Ok(match roots::singular_planes(pair, decide)? {
            Some(planes) => Class::Split { planes },
            None => Class::Other("a singular pencil with no pair of planes"),
        });
    }
    let found = roots::multiple(pair, decide)?;
    let ruled = cylinder_and_cone(pair);
    Ok(match found.as_slice() {
        [] => smooth(pair, decide)?,
        [Multiple::Vertex { phi, at, kind }] => Class::Node { phi: *phi, at: pair.world(*at), kind: *kind },
        [Multiple::Planes { planes }] => Class::Split { planes: *planes },
        [Multiple::AtInfinity { .. }] if ruled => Class::AtInfinity,
        [Multiple::AtInfinity { .. }] => Class::Other("a quartic with its node at infinity on no cylinder"),
        [Multiple::AtInfinity { .. }, Multiple::Vertex { .. }]
        | [Multiple::Vertex { .. }, Multiple::AtInfinity { .. }] => Class::RulingAndCubic,
        [Multiple::Conjugate { line }] => Class::Conjugate { line: *line },
        [Multiple::Double] => Class::Other("surfaces tangent along a conic"),
        _ => Class::Other("more than one degenerate member"),
    })
}

/// The report of a degenerate pencil this method detects and does not parametrize: `intersect.incomplete`, not
/// implemented, with nothing found.
fn not_here() -> Report {
    Report::new(Code::Incomplete, Fault::NotImplemented)
        .with_reason(Reason::PairNotSupported)
        .with_value(NamedValue::count("curves", 0))
        .with_value(NamedValue::count("contacts", 0))
}

impl Method for Pencil {
    fn attempt(
        &self,
        a: &dyn Surface,
        b: &dyn Surface,
        tol: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        let (Some(fa), Some(fb)) = (a.analytic(tol), b.analytic(tol)) else { return Ok(Answer::Declined) };
        let Some(pair) = Pair::new(fa, fb) else { return Ok(Answer::Declined) };
        let (Some(ca), Some(cb)) = (Chart::of(a, fa), Chart::of(b, fb)) else { return Ok(Answer::Declined) };
        work.spend(1)?;
        let mut decide = Decisions::new(tol);
        let answer = match classify(&pair, &mut decide) {
            Ok(Class::Parallel) => Answer::Declined,
            Ok(class) => {
                let ball = fan::Ball::of(&pair, self.extent);
                if let Ok(Err(_)) = emit::built(&pair, class, ([&ca, &cb], ball), &mut decide, section) {
                    section.failures.push(not_here());
                }
                Answer::Answered
            }
            Err(Undecided) => Answer::Answered,
        };
        section.tally = section.tally.merged(decide.tally);
        section.failures.append(&mut decide.failures);
        Ok(answer)
    }

    fn name(&self) -> &'static str {
        "pencil"
    }
}
