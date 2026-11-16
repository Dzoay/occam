//! Source: sarraga-1983
//! The pairs that are not turned about one axis: two planes, a plane with a cylinder or a cone it does not cross square
//! to its axis, and two cylinders whose axes are parallel (Sarraga 1983). A plane meets a plane in a line, a cylinder in
//! an ellipse or in lines along it, and a cone in an ellipse, a parabola or a hyperbola's two branches, or through the
//! apex in rulings or the apex alone. Which, follows from the angle between the plane and the axis and the distance of
//! the axis or the apex from the plane, each of which is decided. Two parallel cylinders are two circles of one
//! cross-section, and meet in lines along their axes where those circles meet.

use occam_math::{Interval, Side, sin_cos};
use occam_units::Vec3;

use super::meet::{Flat, Meeting, Shape, meet};
use super::piece::{Plan, square_to};
use super::{Found, Touch, along, cross3, exact3, norm3, unit};
use crate::decide::{Decisions, Undecided};

/// A plane, as a point on it and the unit normal it faces.
pub(super) type Flatness = (Vec3, Vec3);

/// A surface about an axis: a point on the axis, the axis, and a size, a cylinder's radius or a cone's half-angle.
pub(super) type About = (Vec3, Vec3, f64);

/// Two planes: their line, or none, or the one plane they both are.
pub(super) fn planes((o1, n1): Flatness, (o2, n2): Flatness, decide: &mut Decisions) -> Result<Found, Undecided> {
    let turn = norm3(cross3(exact3(n1), exact3(n2)));
    if decide.angle(turn)? == Side::Within {
        let gap = along(exact3(o2), exact3(o1), exact3(n1));
        return Ok(if decide.length(gap)? == Side::Within { Found::coplanar() } else { Found::default() });
    }
    let c = n1.cross(n2);
    let at = o1 + c.cross(n1) * (n2.dot(o2 - o1) / c.dot(c));
    Ok(Found::of(Plan::Line { at, along: c }))
}

/// The lines along a parallel axis through `p`, at the points of the cross-section `(w, n)` where `one` and `other`
/// meet.
fn lines(
    one: Shape,
    other: Shape,
    (p, w, n, z): (Vec3, Vec3, Vec3, Vec3),
    decide: &mut Decisions,
) -> Result<Found, Undecided> {
    Ok(match meet(&one, &other, decide)? {
        Meeting::Apart => Found::default(),
        Meeting::Same => Found::coincident(),
        Meeting::At(points) => {
            let line = |(at, _): (Flat, bool)| Plan::Line { at: p + w * at.x.mid() + n * at.y.mid(), along: z };
            Found { curves: points.into_iter().map(line).collect(), ..Found::default() }
        }
    })
}

/// A plane and a cylinder it does not cross square to the axis: lines where it runs along the axis, as the plane's line
/// in the cross-section through `p` meets its circle, and else an ellipse, as wide as the cylinder across the axis and
/// longer along the plane's slope by the secant of the angle.
pub(super) fn plane_cylinder(
    (origin, normal): Flatness,
    (on_axis, axis, radius): About,
    decide: &mut Decisions,
) -> Result<Found, Undecided> {
    let cosine = along(exact3(normal), [Interval::exact(0.0); 3], exact3(axis));
    let across = unit(normal.cross(axis));
    if decide.angle(cosine)? == Side::Within {
        let height = along(exact3(origin), exact3(on_axis), exact3(normal));
        let plane = Shape::Line { at: Flat::new(Interval::exact(0.0), height), along: Flat::exact(1.0, 0.0) };
        let section = Shape::Circle { centre: Flat::exact(0.0, 0.0), radius: Interval::exact(radius) };
        return lines(plane, section, (on_axis, across, normal, axis), decide);
    }
    let slant = normal.dot(axis);
    let centre = on_axis + axis * (normal.dot(origin - on_axis) / slant);
    let radii = (radius, radius / slant.abs());
    Ok(Found::of(Plan::Ellipse { centre, x: across, y: normal.cross(across), radii }))
}

/// An interval holding `x`, a value the owned `sin_cos` computed within an ulp.
fn computed(x: f64) -> Interval {
    Interval::new(x, 4.0 * f64::EPSILON * x.abs())
}

/// A plane and a cone it does not cross square to the axis. In the plane, about the foot `f` of the apex at distance
/// `d`, along `e₁`, the plane's steepest direction up the axis, and `e₂` across it, the cone is
/// `K X² + 2 s d c X + cos²α Y² + d² (cos²α − c²) = 0`, where `c` is the cosine between the plane's normal and the axis,
/// `s` its sine, and `K = c² − sin²α`: an ellipse where the plane is steeper than the cone's side, a parabola where it is
/// as steep, and a hyperbola where it is less.
pub(super) fn plane_cone(
    (origin, normal): Flatness,
    (apex, axis, half): About,
    decide: &mut Decisions,
) -> Result<Found, Undecided> {
    let (sin, cos) = sin_cos(half);
    let cosine = along(exact3(normal), [Interval::exact(0.0); 3], exact3(axis));
    let gap = along(exact3(apex), exact3(origin), exact3(normal));
    let steep = if cosine.mid() < 0.0 { -cosine } else { cosine } - computed(sin);
    let kind = decide.angle(steep)?;
    let (slant, d) = (cosine.mid(), gap.mid());
    let e1 = unit(axis - normal * slant);
    let e2 = normal.cross(e1);
    if decide.length(gap)? == Side::Within {
        return Ok(through_apex(kind, apex, (e1, e2), cos / e1.dot(axis)));
    }
    let (rise, k, foot) = (e1.dot(axis), slant * slant - sin * sin, apex - normal * d);
    let centre = foot + e1 * (-rise * d * slant / k);
    Ok(match kind {
        Side::Above => {
            let radii = (d.abs() * sin * cos / k, d.abs() * sin / k.sqrt());
            Found::of(Plan::Ellipse { centre, x: e1, y: e2, radii })
        }
        Side::Within => {
            let vertex = foot + e1 * (-d * (cos * cos - slant * slant) / (2.0 * rise * slant));
            let focal = rise * (d * slant).abs() / (2.0 * cos * cos);
            let way = if d * slant > 0.0 { -1.0 } else { 1.0 };
            Found::of(Plan::Parabola { vertex, x: e1 * way, y: e2 * way, focal })
        }
        Side::Below => {
            let radii = (d.abs() * sin * cos / -k, d.abs() * sin / (-k).sqrt());
            let branch = |way: f64| Plan::Hyperbola { centre, x: e1 * way, y: e2 * way, radii };
            Found { curves: vec![branch(1.0), branch(-1.0)], ..Found::default() }
        }
    })
}

/// A plane through a cone's apex `a`: the apex alone where the plane is steeper than the cone's side, the ruling along
/// its steepest direction where it is as steep, and two rulings where it is less, at the cone's angle from the axis,
/// `ratio` of the way along `e₁` and the rest across, which cross at the apex, a contact. The rulings have no ends, so
/// they are not cut there.
fn through_apex(kind: Side, a: Vec3, (e1, e2): (Vec3, Vec3), ratio: f64) -> Found {
    match kind {
        Side::Above => Found { contacts: vec![Touch::point(a)], ..Found::default() },
        Side::Within => Found::of(Plan::Ruling { apex: a, along: e1 }),
        Side::Below => {
            let across = ((1.0 - ratio) * (1.0 + ratio)).sqrt();
            let ruling = |way: f64| Plan::Ruling { apex: a, along: e1 * ratio + e2 * (across * way) };
            Found { curves: vec![ruling(1.0), ruling(-1.0)], contacts: vec![Touch::point(a)], ..Found::default() }
        }
    }
}

/// Two cylinders whose axes are parallel and apart, as their circles in the cross-section through the first's axis.
pub(super) fn cylinders((p1, z, r1): About, (p2, _, r2): About, decide: &mut Decisions) -> Result<Found, Undecided> {
    let w = square_to(z);
    let n = z.cross(w);
    let (from, to) = (exact3(p1), exact3(p2));
    let other = Flat::new(along(to, from, exact3(w)), along(to, from, exact3(n)));
    let one = Shape::Circle { centre: Flat::exact(0.0, 0.0), radius: Interval::exact(r1) };
    let two = Shape::Circle { centre: other, radius: Interval::exact(r2) };
    lines(one, two, (p1, w, n, z), decide)
}
