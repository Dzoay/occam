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
use occam_math::Side;
use occam_units::{Direction, ParamKind, Point, Vec3};

use super::quadric::{Natural, Ruling};
use super::{Pair, cut};
use crate::closed::{Found, Plan, cross3, exact3, minus, norm3, solve, unit};
use crate::decide::{Decisions, Undecided};

/// Points of each conic or line, in the world, to hold it to the other surface: at parameters spread over a closed
/// conic's turn, and along an open curve from its vertex, a parameter that is a length taken in the pair's.
fn samples(plan: &Plan, length: f64) -> Vec<Vec3> {
    plan.curve().map_or_else(Vec::new, |c| {
        let unit = if c.range().kind == ParamKind::Length { length } else { 1.0 };
        [0.3, 0.9, 1.9, 2.6].map(|t| c.point(t * unit).to_vec3()).to_vec()
    })
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
    for p in found.curves.iter().flat_map(|plan| samples(plan, pair.length())) {
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

/// The points of the line where the two planes meet that lie on both surfaces: where a member of rank two has
/// conjugate planes, the real points of the curve.
pub(super) fn touches(pair: &Pair, planes: [(Vec3, Vec3); 2], decide: &mut Decisions) -> Result<Vec<Vec3>, Undecided> {
    let (r, o) = cut(pair);
    let mut out = Vec::new();
    for at in crossings(pair, r, planes, decide)? {
        if pair.quadric(o).holds(at - pair.origin, decide)? == Side::Within {
            out.push(at);
        }
    }
    Ok(out)
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
    let mut candidates = alpha.roots();
    candidates.extend(alpha.rate().roots());
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

#[cfg(test)]
mod tests {
    use super::super::tests::{ball, unplaced, upright};
    use super::*;
    use crate::closed::tests::{O, X, Z, tol, v};

    fn decide() -> Decisions {
        Decisions::new(tol())
    }

    /// Whether `got` are `want`, within 1e-12.
    fn near(got: &[Vec3], want: &[Vec3]) -> bool {
        got.len() == want.len() && got.iter().zip(want).all(|(g, w)| (*g - *w).norm() <= 1e-12)
    }

    #[test]
    fn a_line_is_sampled_along_lengths_of_the_pair_and_a_circle_round_its_turn() {
        let line = Plan::Line { at: O, along: X };
        for length in [1e-3, 1e5] {
            let along: Vec<f64> = samples(&line, length).iter().map(|p| p.x / length).collect();
            assert!(along.iter().zip([0.3, 0.9, 1.9, 2.6]).all(|(a, b)| (a - b).abs() <= 1e-12), "{length}: {along:?}");
        }
        let circle = Plan::Circle { centre: O, normal: Z, radius: 2.0 };
        let round = samples(&circle, 1e5);
        assert!(round.iter().all(|p| (p.norm() - 2.0).abs() <= 1e-12) && round.len() == 4, "{round:?}");
    }

    #[test]
    fn a_split_holds_where_its_conics_lie_on_the_other_surface_and_not_where_they_miss_it_by_more_than_the_band() {
        let planes = [(Z, v(0.0, 0.0, 0.8)), (Z, v(0.0, 0.0, -0.8))];
        assert!(holds(&unplaced(upright(O, Z, 0.6), ball(O, 1.0)), planes, &mut decide()).unwrap());
        let leaning = unplaced(upright(O, v(1e-9, 0.0, 1.0), 0.6), ball(O, 1.0));
        assert!(!holds(&leaning, planes, &mut decide()).unwrap(), "a nanoradian off");
    }

    #[test]
    fn the_planes_common_line_crosses_a_surface_where_its_quadric_vanishes_along_it() {
        // The cylinder of radius 1 about Z, with a sphere about (2, 0, 4), so that the pair's frame is about (1, 0, 2).
        let pair = unplaced(upright(O, Z, 1.0), ball(v(2.0, 0.0, 4.0), 1.0));
        let through = |p: Vec3, along: Vec3| {
            let n1 = unit(along.cross(Z));
            let n2 = unit(along.cross(n1));
            [(n1, p - pair.origin), (n2, p - pair.origin)]
        };
        // The line through `p` along `u` meets it where `|(p + t u)ₓᵧ|² = 1`.
        let (p, u) = (v(0.3, -0.4, 0.7), unit(v(1.0, 2.0, 0.5)));
        let quadratic = [u.x * u.x + u.y * u.y, p.x * u.x + p.y * u.y, p.x * p.x + p.y * p.y - 1.0];
        let [bend, slope, value] = quadratic;
        let at = |sign: f64| p + u * ((-slope + sign * (slope * slope - bend * value).sqrt()) / bend);
        let got = crossings(&pair, 0, through(p, u), &mut decide()).unwrap();
        assert!(near(&got, &[at(-1.0), at(1.0)]), "{got:?}");
        let tangent = unit(v(0.0, 1.0, 0.5));
        let got = crossings(&pair, 0, through(v(1.0, 0.0, 0.3), tangent), &mut decide()).unwrap();
        assert!(near(&got, &[v(1.0, 0.0, 0.3)]), "touching it: {got:?}");
        assert_eq!(crossings(&pair, 0, through(v(2.0, 0.0, 0.3), tangent), &mut decide()).unwrap(), Vec::new());
    }

    #[test]
    fn where_conjugate_planes_meet_the_curve_has_its_only_real_points() {
        // The cylinder of radius 1 about Z and the sphere of radius 1 about (2, 0, 0) touch at (1, 0, 0).
        let pair = unplaced(upright(v(0.0, 0.0, 5.0), Z, 1.0), ball(v(2.0, 0.0, 0.0), 1.0));
        let along_y = |p: Vec3| [(X, p - pair.origin), (Z, p - pair.origin)];
        assert!(near(&touches(&pair, along_y(X), &mut decide()).unwrap(), &[X]));
        let off = touches(&pair, along_y(v(0.5, 0.0, 0.0)), &mut decide()).unwrap();
        assert!(off.is_empty(), "crossing the cylinder off the sphere: {off:?}");
    }
}
