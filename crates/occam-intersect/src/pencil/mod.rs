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
//! Singular points are kept as contacts. A degenerate pencil of any other kind is reported, not fitted.

mod eigen;
mod emit;
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
use occam_units::{Tolerance, Vec3};

use self::quadric::{Natural, Quadric, Ruling};
use self::roots::{Multiple, Singular};
use crate::closed::{cross3, exact3, minus, norm3};
use crate::decide::{Decisions, Undecided};
use crate::methods::{Answer, Method, Spent, Work};
use crate::section::Section;

/// Levin's pencil method, which answers the pairs of spheres, cylinders and cones the closed forms leave.
pub struct Pencil;

/// What a pencil is, as its classification finds it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    /// Four simple roots: a smooth quartic, in as many loops as it has.
    Smooth,
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

    #[cfg(test)]
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

/// A smooth quartic, unless the ruled surface is a cone some of whose rulings run along the other's asymptotic cone,
/// so that the curve reaches infinity.
fn smooth(pair: &Pair) -> Class {
    let Some((_, o, ruling)) = pair.ruled() else { return Class::Other("no ruled member") };
    let alpha = ruling.along(&pair.quadric(o)).alpha;
    if alpha.roots().is_empty() { Class::Smooth } else { Class::Other("a quartic that reaches infinity") }
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
    Ok(match found.as_slice() {
        [] => smooth(pair),
        [Multiple::Vertex { phi, at, kind }] => Class::Node { phi: *phi, at: pair.world(*at), kind: *kind },
        [Multiple::Planes { planes }] => Class::Split { planes: *planes },
        [Multiple::AtInfinity { .. }] => Class::Other("a quartic with its node at infinity"),
        [Multiple::AtInfinity { .. }, Multiple::Vertex { .. }]
        | [Multiple::Vertex { .. }, Multiple::AtInfinity { .. }] => Class::Other("a ruling and a cubic"),
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
                if let Ok(Err(_)) = emit::built(&pair, class, [&ca, &cb], &mut decide, section) {
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
