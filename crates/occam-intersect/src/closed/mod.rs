//! Source: patrikalakis-maekawa-2002
//! The closed forms (P5; Patrikalakis and Maekawa 2002, ch. 5): where both surfaces present an exact form and the pair
//! has a section in closed form, it is computed, not traced. A plane meets a plane, sphere, cylinder or cone in a line
//! or a conic; surfaces turned about one axis, a torus among them, meet in circles where their meridians cross, which
//! covers a plane across the axis, two spheres and a sphere centred on the axis; and two parallel cylinders meet in
//! lines. Every other pair declines: a sphere, cone or cylinder with a cylinder off their common axis goes to Levin's
//! pencil method ([`crate::Pencil`]), and a torus with most surfaces to the hybrid.
//!
//! Each curve is one of occam-geom's exact families, with its image on both surfaces in their own parameters, read
//! through a chart of each that is recovered from its evaluation. A point where the surfaces touch is a contact, and
//! coincident surfaces are found (O25). Every decision is made on certified intervals through occam-math's predicates,
//! so a tangency within rounding of the band is a contact (P5's hull test rejected one for want of a rounding margin),
//! and one the band cannot make is reported as `decide.undecided`, not guessed.

mod coaxial;
mod conic;
mod meet;
mod piece;
#[cfg(test)]
pub(crate) mod tests;

use occam_geom::{Analytic, Chart, Surface};
use occam_math::{Interval, Side};
use occam_units::{Point, Tolerance, Vec3};

use self::coaxial::{Axis, coaxial};
pub(crate) use self::piece::{Plan, square_to};
use crate::decide::{Decisions, Undecided};
use crate::methods::{Answer, Method, Spent, Work};
use crate::section::{Coincidence, Contact, Section, SectionCurve};

/// The closed forms, which answer the pairs that have one and decline the rest.
pub struct Closed;

/// What a closed form found, in space.
#[derive(Default)]
pub(crate) struct Found {
    pub(crate) curves: Vec<Plan>,
    pub(crate) contacts: Vec<Vec3>,
    pub(crate) coincidence: Option<Coincidence>,
}

impl Found {
    fn of(plan: Plan) -> Self {
        Self { curves: vec![plan], ..Self::default() }
    }

    fn coincident() -> Self {
        Self { coincidence: Some(Coincidence::Coaxial), ..Self::default() }
    }

    fn coplanar() -> Self {
        Self { coincidence: Some(Coincidence::Coplanar), ..Self::default() }
    }
}

pub(crate) fn unit(v: Vec3) -> Vec3 {
    v * (1.0 / v.norm())
}

/// `v`'s parts, each exactly.
pub(crate) fn exact3(v: Vec3) -> [Interval; 3] {
    [v.x, v.y, v.z].map(Interval::exact)
}

/// `p − from`, certified.
pub(crate) fn minus([px, py, pz]: [Interval; 3], [fx, fy, fz]: [Interval; 3]) -> [Interval; 3] {
    [px - fx, py - fy, pz - fz]
}

/// `(p − from) · towards`, certified.
pub(crate) fn along(p: [Interval; 3], from: [Interval; 3], [tx, ty, tz]: [Interval; 3]) -> Interval {
    let [x, y, z] = minus(p, from);
    x * tx + y * ty + z * tz
}

pub(crate) fn cross3([ax, ay, az]: [Interval; 3], [bx, by, bz]: [Interval; 3]) -> [Interval; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

pub(crate) fn norm3([x, y, z]: [Interval; 3]) -> Interval {
    (x * x + y * y + z * z).sqrt()
}

/// The axis a surface is turned about, for a cylinder, a cone or a torus.
fn axis_of(form: Analytic) -> Option<Axis> {
    match form {
        Analytic::Cylinder { on_axis: at, axis, .. }
        | Analytic::Cone { apex: at, axis, .. }
        | Analytic::Torus { centre: at, axis, .. } => Some(Axis { at: at.to_vec3(), along: axis.get() }),
        Analytic::Plane { .. } | Analytic::Sphere { .. } => None,
    }
}

/// The order the pairs are written in: a plane, a sphere, a cylinder, a cone, a torus.
const fn rank(form: Analytic) -> u8 {
    match form {
        Analytic::Plane { .. } => 0,
        Analytic::Sphere { .. } => 1,
        Analytic::Cylinder { .. } => 2,
        Analytic::Cone { .. } => 3,
        Analytic::Torus { .. } => 4,
    }
}

/// Whether the unit `direction` runs along `axis`, within the angle band.
fn parallel(direction: Vec3, axis: &Axis, decide: &mut Decisions) -> Result<bool, Undecided> {
    Ok(decide.angle(norm3(cross3(exact3(direction), exact3(axis.along))))? == Side::Within)
}

/// Whether `p` is on `axis`, within the length band.
fn on(p: Vec3, axis: &Axis, decide: &mut Decisions) -> Result<bool, Undecided> {
    let off = cross3(minus(exact3(p), exact3(axis.at)), exact3(axis.along));
    Ok(decide.length(norm3(off))? == Side::Within)
}

/// Whether `form` is turned about `axis`: a plane across it, a sphere centred on it, or a surface about it.
fn about(form: Analytic, axis: &Axis, decide: &mut Decisions) -> Result<bool, Undecided> {
    match form {
        Analytic::Plane { normal, .. } => parallel(normal.get(), axis, decide),
        Analytic::Sphere { centre, .. } => on(centre.to_vec3(), axis, decide),
        _ => {
            let Some(own) = axis_of(form) else { return Ok(false) };
            Ok(parallel(own.along, axis, decide)? && on(own.at, axis, decide)?)
        }
    }
}

/// Two spheres: about the line of their centres, or, where those are one, the one sphere or none.
fn spheres(first: Analytic, second: Analytic, decide: &mut Decisions) -> Result<Found, Undecided> {
    let (Analytic::Sphere { centre, radius }, Analytic::Sphere { centre: other, radius: size }) = (first, second)
    else {
        return Ok(Found::default());
    };
    let (c1, c2) = (centre.to_vec3(), other.to_vec3());
    if decide.length(norm3(minus(exact3(c2), exact3(c1))))? == Side::Within {
        let same = Interval::exact(radius.millimetres()) - Interval::exact(size.millimetres());
        return Ok(if decide.length(same)? == Side::Within { Found::coincident() } else { Found::default() });
    }
    coaxial(first, second, &Axis { at: c1, along: unit(c2 - c1) }, decide)
}

/// The section of a pair in rank order, or `Ok(None)` where it has no closed form here.
pub(crate) fn solve(first: Analytic, second: Analytic, decide: &mut Decisions) -> Result<Option<Found>, Undecided> {
    Ok(Some(match (first, second) {
        (Analytic::Plane { origin, normal }, Analytic::Plane { origin: other, normal: facing }) => {
            conic::planes((origin.to_vec3(), normal.get()), (other.to_vec3(), facing.get()), decide)?
        }
        (Analytic::Plane { normal, .. }, Analytic::Sphere { centre, .. }) => {
            coaxial(first, second, &Axis { at: centre.to_vec3(), along: normal.get() }, decide)?
        }
        (Analytic::Sphere { .. }, Analytic::Sphere { .. }) => spheres(first, second, decide)?,
        (Analytic::Plane { .. } | Analytic::Sphere { .. }, _) => {
            let Some(axis) = axis_of(second) else { return Ok(None) };
            if !about(first, &axis, decide)? {
                return off_axis(first, second, decide);
            }
            coaxial(first, second, &axis, decide)?
        }
        _ => {
            let (Some(axis), Some(other)) = (axis_of(first), axis_of(second)) else { return Ok(None) };
            if about(second, &axis, decide)? {
                coaxial(first, second, &axis, decide)?
            } else {
                return parallel_cylinders(first, second, (&axis, &other), decide);
            }
        }
    }))
}

/// Two cylinders whose axes are parallel but apart; any other pair of surfaces about two axes has no closed form here.
fn parallel_cylinders(
    first: Analytic,
    second: Analytic,
    (axis, other): (&Axis, &Axis),
    decide: &mut Decisions,
) -> Result<Option<Found>, Undecided> {
    let (Analytic::Cylinder { radius, .. }, Analytic::Cylinder { radius: size, .. }) = (first, second) else {
        return Ok(None);
    };
    if !parallel(other.along, axis, decide)? {
        return Ok(None);
    }
    let (one, two) = ((axis.at, axis.along, radius.millimetres()), (other.at, other.along, size.millimetres()));
    Ok(Some(conic::cylinders(one, two, decide)?))
}

/// A plane with a cylinder or a cone that it does not cross square to the axis; any other pair off a common axis has
/// no closed form here.
fn off_axis(first: Analytic, second: Analytic, decide: &mut Decisions) -> Result<Option<Found>, Undecided> {
    let Analytic::Plane { origin, normal } = first else { return Ok(None) };
    let plane = (origin.to_vec3(), normal.get());
    Ok(Some(match second {
        Analytic::Cylinder { on_axis, axis, radius } => {
            conic::plane_cylinder(plane, (on_axis.to_vec3(), axis.get(), radius.millimetres()), decide)?
        }
        Analytic::Cone { apex, axis, half_angle } => {
            conic::plane_cone(plane, (apex.to_vec3(), axis.get(), half_angle.radians()), decide)?
        }
        _ => return Ok(None),
    }))
}

/// Puts what was found into `section`, each curve with its images on both surfaces, or gives `false`, having put
/// nothing, where a curve or an image cannot be made.
pub(crate) fn emit(
    found: Found,
    charts: [&Chart; 2],
    decide: &mut Decisions,
    section: &mut Section,
) -> Result<bool, Undecided> {
    let mut curves = Vec::new();
    for plan in found.curves {
        let Some(made) = piece::pieces(plan, charts, decide)? else { return Ok(false) };
        curves.extend(made.into_iter().map(|m| {
            let [on_a, on_b] = m.on;
            SectionCurve { curve: m.curve, on_a, on_b }
        }));
    }
    section.curves.extend(curves);
    section.contacts.extend(found.contacts.into_iter().map(|at| Contact::Point {
        at: Point::from_vec3(at),
        on_a: charts[0].parameters(Point::from_vec3(at)),
        on_b: charts[1].parameters(Point::from_vec3(at)),
    }));
    section.coincidence = found.coincidence;
    Ok(true)
}

impl Method for Closed {
    fn name(&self) -> &'static str {
        "closed"
    }

    fn attempt(
        &self,
        a: &dyn Surface,
        b: &dyn Surface,
        tol: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        let (Some(fa), Some(fb)) = (a.analytic(tol), b.analytic(tol)) else { return Ok(Answer::Declined) };
        let (Some(ca), Some(cb)) = (Chart::of(a, fa), Chart::of(b, fb)) else { return Ok(Answer::Declined) };
        work.spend(1)?;
        let mut decide = Decisions::new(tol);
        let solved = if rank(fb) < rank(fa) { solve(fb, fa, &mut decide) } else { solve(fa, fb, &mut decide) };
        let emitted = solved.and_then(|found| found.map_or(Ok(false), |f| emit(f, [&ca, &cb], &mut decide, section)));
        let answer = if matches!(emitted, Ok(false)) { Answer::Declined } else { Answer::Answered };
        section.tally = section.tally.merged(decide.tally);
        section.failures.append(&mut decide.failures);
        Ok(answer)
    }
}
