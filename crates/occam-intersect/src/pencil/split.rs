//! Source: wang-goldman-tu-2003
//! Pencils that split into lines and conics (Wang, Goldman and Tu 2003). A member of rank two is a pair of planes, and
//! each cuts the curve in a conic of either surface, or in a line where it is tangent to one along a ruling, as where
//! a cylinder is tangent to a cone along the ruling through its apex. The split is decided by holding the conics one
//! surface gives to the other, point by point against the band, and the conics are then the closed forms' sections of
//! the planes. Where the planes' common line crosses the surfaces, the conics cross, and those points are contacts;
//! they may be complex, as for two circles of a sphere, and then there are none. Where they are real they are where a
//! split nudged off by a little comes nearest to one, so they are held to the other surface too. Two cones with one apex meet in the
//! rulings they share, the rulings of one along the other's asymptotic cone.

use occam_geom::Analytic;
use occam_math::{Side, sin_cos, sinh_cosh};
use occam_units::{Direction, Point, Vec3};

use super::quadric::{Natural, Ruling};
use super::{Pair, cut};
use crate::closed::{Found, Plan, cross3, exact3, minus, norm3, solve, square_to, unit};
use crate::decide::{Decisions, Undecided};

/// Points of each conic or line, in the world, to hold it to the other surface.
fn samples(plan: &Plan) -> Vec<Vec3> {
    let turn = [0.3, 1.9, 3.4, 5.0].map(sin_cos);
    match *plan {
        Plan::Line { at, along } | Plan::Ruling { apex: at, along } => {
            vec![at, at + unit(along), at - unit(along) * 2.0]
        }
        Plan::Circle { centre, normal, radius } => {
            let x = square_to(unit(normal));
            let y = unit(normal).cross(x);
            turn.iter().map(|(s, c)| centre + (x * *c + y * *s) * radius).collect()
        }
        Plan::Ellipse { centre, x, y, radii } => {
            turn.iter().map(|(s, c)| centre + x * (radii.0 * c) + y * (radii.1 * s)).collect()
        }
        Plan::Parabola { vertex, x, y, focal } => {
            [-1.0, 0.0, 1.5].iter().map(|t| vertex + x * (t * t / (4.0 * focal)) + y * *t).collect()
        }
        Plan::Hyperbola { centre, x, y, radii } => [-0.7, 0.0, 0.9]
            .map(sinh_cosh)
            .iter()
            .map(|(sinh, cosh)| centre + x * (radii.0 * cosh) + y * (radii.1 * sinh))
            .collect(),
    }
}

/// The section of the plane through `at` facing `normal`, both in the pair's frame, with surface `r`, every point of
/// it held to surface `o`: `Ok(None)` where it has no closed form or a point is off `o`.
fn section(
    pair: &Pair,
    (r, o): (usize, usize),
    (normal, at): (Vec3, Vec3),
    decide: &mut Decisions,
) -> Result<Option<Found>, Undecided> {
    let plane = Analytic::Plane { origin: Point::from_vec3(pair.world(at)), normal: Direction::normalised(normal) };
    let Some(found) = solve(plane, pair.form(r), decide)? else {
        return Ok(None);
    };
    let other = pair.quadric(o);
    for p in found.curves.iter().flat_map(samples) {
        if other.holds(p - pair.origin, decide)? != Side::Within {
            return Ok(None);
        }
    }
    Ok(Some(found))
}

/// Whether the member whose planes are `planes` is a member of the pencil within the band: where the planes' common
/// line crosses one surface, the conics' crossings, it is on the other, and the conics each plane cuts from the one lie
/// on the other, at least one of them real.
pub(super) fn holds(pair: &Pair, planes: [(Vec3, Vec3); 2], decide: &mut Decisions) -> Result<bool, Undecided> {
    let (r, o) = cut(pair);
    for at in crossings(pair, r, planes, decide)? {
        if pair.quadric(o).holds(at - pair.origin, decide)? != Side::Within {
            return Ok(false);
        }
    }
    let mut real = false;
    for plane in planes {
        let Some(found) = section(pair, cut(pair), plane, decide)? else { return Ok(false) };
        real |= !found.curves.is_empty();
    }
    Ok(real)
}

/// The point of the two planes' common line along `u` nearest the origin.
fn on_both([(n1, p1), (n2, p2)]: [(Vec3, Vec3); 2], u: Vec3) -> Vec3 {
    let (d1, d2) = (n1.dot(p1), n2.dot(p2));
    (n2.cross(u) * d1 + u.cross(n1) * d2) * (1.0 / n1.cross(n2).dot(u))
}

/// Where the two planes' common line crosses surface `r`: one point where it touches, decided at its foot, where the
/// quadric is least or most along it, as a distance there; else two points or none, as its discriminant says.
fn crossings(pair: &Pair, r: usize, planes: [(Vec3, Vec3); 2], decide: &mut Decisions) -> Result<Vec<Vec3>, Undecided> {
    let line = planes[0].0.cross(planes[1].0);
    if line.norm() <= 1e-9 {
        return Ok(Vec::new());
    }
    let line = unit(line);
    let (base, quadric) = (on_both(planes, line), pair.quadric(r));
    let (bend, slope, value) = (quadric.form(line, line), line.dot(quadric.half_gradient(base)), quadric.value(base));
    let foot = base + line * (-slope / bend);
    if quadric.holds(foot, decide)? == Side::Within {
        return Ok(vec![pair.world(foot)]);
    }
    let disc = slope * slope - bend * value;
    if disc <= 0.0 {
        return Ok(Vec::new());
    }
    let half = disc.sqrt() / bend.abs();
    Ok(vec![pair.world(foot - line * half), pair.world(foot + line * half)])
}

/// `plan`, or, where it is a line through a cone's apex within the band, that cone's ruling, whose image on the cone
/// is a line of constant angle through the apex rather than one that jumps there.
fn ruled(pair: &Pair, plan: Plan, decide: &mut Decisions) -> Result<Plan, Undecided> {
    let Plan::Line { at, along } = plan else { return Ok(plan) };
    for surface in &pair.surfaces {
        if let Natural::Cone { apex, .. } = *surface {
            let off = norm3(cross3(minus(exact3(apex), exact3(at)), exact3(unit(along))));
            if decide.length(off)? == Side::Within {
                return Ok(Plan::Ruling { apex, along });
            }
        }
    }
    Ok(plan)
}

/// The conics of the member whose planes are `planes`, and where they cross: `Ok(Err)` where a plane has no section.
pub(super) fn conics(
    pair: &Pair,
    planes: [(Vec3, Vec3); 2],
    decide: &mut Decisions,
) -> Result<Result<Found, &'static str>, Undecided> {
    let (r, o) = cut(pair);
    let mut found = Found { contacts: crossings(pair, r, planes, decide)?, ..Found::default() };
    for plane in planes {
        let Some(part) = section(pair, (r, o), plane, decide)? else { return Ok(Err("a plane with no conic")) };
        for plan in part.curves {
            found.curves.push(ruled(pair, plan, decide)?);
        }
    }
    Ok(Ok(found))
}

/// Two cones with one apex: the rulings of the first along the second's asymptotic cone, and the apex as a contact.
pub(super) fn rulings(pair: &Pair, decide: &mut Decisions) -> Result<Result<Found, &'static str>, Undecided> {
    let (Some(first), Natural::Cone { .. }) = (Ruling::of(&pair.surfaces[0], pair.origin), pair.surfaces[1]) else {
        return Ok(Err("not two cones"));
    };
    let alpha = first.along(&pair.quadric(1)).alpha;
    if alpha.0.iter().all(|c| c.abs() <= 1e-13) {
        return Ok(Err("one cone"));
    }
    let small = 1e-14 * alpha.size();
    let mut candidates = alpha.roots(small);
    candidates.extend(alpha.rate().roots(small));
    let apex = pair.surfaces[0].at();
    let mut found = Found { contacts: vec![apex], ..Found::default() };
    let mut ways: Vec<Vec3> = Vec::new();
    for theta in candidates {
        let g = first.direction(theta);
        if pair.quadric(1).toward(g, decide)? != Side::Within || ways.iter().any(|w| w.cross(g).norm() <= 1e-9) {
            continue;
        }
        ways.push(g);
        found.curves.push(Plan::Ruling { apex, along: g });
    }
    Ok(Ok(found))
}
