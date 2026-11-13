//! Source: original
//! What the cylinder, the cone, the sphere and the torus share: each turns a meridian about its placement's Z axis, so
//! its point is `origin + ρ(v) e(u) + ζ(v) z`, with `e(u) = cos u x + sin u y`. The partial derivative `i` times in `u`
//! and `j` times in `v` is `ρ⁽ʲ⁾(v) e⁽ⁱ⁾(u)`, plus `ζ⁽ʲ⁾(v) z` where `i` is 0: one formula for every order, which the
//! rungs and `derivatives` both call, so they agree bit for bit.

use occam_math::{Real, atan2, sin_cos};
use occam_units::{Bounds, Length, Point, Tangent, UvPoint, Vec3, World};

use crate::axes::Axes;
use crate::batch::{Jet1, Jet2};
use crate::domain::UvBox;
use crate::support::{everything, splat};
use crate::turn::{above, larger, quarter_turns, smaller, swing};

/// Where a surface turned about an axis is, in a plane through the axis, as `v` runs: `ρ` across the axis and `ζ` along
/// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Meridian {
    /// `ρ = start + v slope.0` and `ζ = v slope.1`: a straight line, `slope` a unit direction and `v` a length along it.
    Straight {
        /// `ρ` where `v` is 0.
        start: f64,
        /// The line's direction.
        slope: (f64, f64),
    },
    /// `ρ = centre + radius cos v` and `ζ = radius sin v`: a circle, `v` an angle, which never crosses the axis.
    Round {
        /// How far its centre is from the axis.
        centre: f64,
        /// Its radius.
        radius: f64,
    },
}

/// A meridian at one `v`, with what its derivatives need.
#[derive(Clone, Copy)]
enum At<R> {
    /// A straight meridian's `ρ` and `ζ` there, and its direction.
    Straight { rho: R, zeta: R, slope: (f64, f64) },
    /// A round meridian's centre and radius, and `cos v` and `sin v`.
    Round { centre: f64, radius: f64, cos: R, sin: R },
}

impl Meridian {
    /// The meridian at `v`.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn at<R: Real>(self, v: R) -> At<R> {
        match self {
            Self::Straight { start, slope } => {
                At::Straight { rho: R::splat(start) + v * R::splat(slope.0), zeta: v * R::splat(slope.1), slope }
            }
            Self::Round { centre, radius } => {
                let (sin, cos) = sin_cos(v);
                At::Round { centre, radius, cos, sin }
            }
        }
    }
}

impl<R: Real> At<R> {
    /// `ρ` and `ζ` differentiated `j` times in `v`.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn rates(self, j: u8) -> (R, R) {
        match self {
            Self::Straight { rho, zeta, slope } => match j {
                0 => (rho, zeta),
                1 => (R::splat(slope.0), R::splat(slope.1)),
                _ => (R::splat(0.0), R::splat(0.0)),
            },
            Self::Round { centre, radius, cos, sin } => {
                let (along_rho, along_zeta) = quarter_turns(cos, sin, j);
                let (radius, rho) = (R::splat(radius), R::splat(radius) * along_rho);
                (if j == 0 { R::splat(centre) + rho } else { rho }, radius * along_zeta)
            }
        }
    }
}

/// The partial derivative `i` times in `u` and `j` in `v`, given `(cos u, sin u)`: where both are 0, the point less the
/// origin.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn partial<R: Real>(axes: &Axes, turn: (R, R), at: At<R>, (i, j): (u8, u8)) -> Vec3<R> {
    let (rho, zeta) = at.rates(j);
    let (along_x, along_y) = quarter_turns(turn.0, turn.1, i);
    let across = axes.along(rho * along_x, rho * along_y);
    if i == 0 { across + splat(axes.normal.get()) * zeta } else { across }
}

/// What an evaluation at `uv` needs: `(cos u, sin u)` and the meridian at `v`.
#[inline(always)]
fn prepare<R: Real>(meridian: Meridian, uv: UvPoint<R>) -> ((R, R), At<R>) {
    let (sin, cos) = sin_cos(uv.u());
    ((cos, sin), meridian.at(uv.v()))
}

/// The point, given `(cos u, sin u)` and the meridian at `v`.
#[inline(always)]
fn located<R: Real>(axes: &Axes, turn: (R, R), at: At<R>) -> Point<World, R> {
    Point::from_vec3(splat(axes.origin) + partial(axes, turn, at, (0, 0)))
}

/// The point at `uv` of the surface turned from `meridian` about `axes`.
#[inline(always)]
pub(crate) fn point<R: Real>(axes: &Axes, meridian: Meridian, uv: UvPoint<R>) -> Point<World, R> {
    let (turn, at) = prepare(meridian, uv);
    located(axes, turn, at)
}

/// The point and its first derivatives.
#[inline(always)]
pub(crate) fn first<R: Real>(axes: &Axes, meridian: Meridian, uv: UvPoint<R>) -> Jet1<R> {
    let (turn, at) = prepare(meridian, uv);
    let rate = |order| Tangent::new(partial(axes, turn, at, order));
    Jet1 { point: located(axes, turn, at), du: rate((1, 0)), dv: rate((0, 1)) }
}

/// The point and its first and second derivatives.
#[inline(always)]
pub(crate) fn second<R: Real>(axes: &Axes, meridian: Meridian, uv: UvPoint<R>) -> Jet2<R> {
    let (turn, at) = prepare(meridian, uv);
    let rate = |order| Tangent::new(partial(axes, turn, at, order));
    let first = Jet1 { point: located(axes, turn, at), du: rate((1, 0)), dv: rate((0, 1)) };
    Jet2 { first, duu: rate((2, 0)), duv: rate((1, 1)), dvv: rate((0, 2)) }
}

/// Appends every partial derivative of total degree 1 to `order` at `uv`, by degree and from all `u` to all `v`.
pub(crate) fn derivatives(axes: &Axes, meridian: Meridian, uv: UvPoint, order: u8, out: &mut Vec<Tangent<World>>) {
    let (turn, at) = prepare(meridian, uv);
    for degree in 1..=order {
        for (i, j) in (0..=degree).rev().zip(0..=degree) {
            out.push(Tangent::new(partial(axes, turn, at, (i, j))));
        }
    }
}

/// `p` about the axis: how far it is from it, its angle about it from `x`, from 0 to 2π, and its height along it.
pub(crate) fn cylindrical(axes: &Axes, p: Point<World>) -> (f64, f64, f64) {
    let local = axes.local(p);
    let across = (local.x * local.x + local.y * local.y).sqrt();
    (across, above(atan2(local.y, local.x), 0.0), local.z)
}

/// The angle about the axis, from 0 to 2π, of the direction opposite `p`'s across it: that of the line through the axis
/// on `p`'s far side. It is found as itself, not as `p`'s angle and half a turn, which `above` would take the same
/// whether the half turn were added or taken away.
pub(crate) fn opposite(axes: &Axes, p: Point<World>) -> f64 {
    let local = axes.local(p);
    above(atan2(-local.y, -local.x), 0.0)
}

/// The box of the surface over `patch`, rounding included. Each coordinate is `o + ρ(v) e(u) + ζ(v) n`, where `e` and
/// `n` are that coordinate of `e(u)` and of the axis: the extremes of `e` over the patch's `u` come from [`swing`], and
/// then the meridian's over its `v`, by [`along_line`] or [`along_circle`]. A patch that is not finite gives all of
/// space.
pub(crate) fn turned_box(axes: &Axes, meridian: Meridian, patch: UvBox) -> Bounds {
    let (lo, hi) = (patch.lo, patch.hi);
    if ![lo.u(), lo.v(), hi.u(), hi.v()].into_iter().all(f64::is_finite) {
        return everything();
    }
    let (us, vs) = ((lo.u(), hi.u()), (lo.v(), hi.v()));
    let (start, end) = (sin_cos(lo.u()), sin_cos(hi.u()));
    let extent = |o: f64, x: f64, y: f64, n: f64| {
        let ends = (x * start.1 + y * start.0, x * end.1 + y * end.0);
        let across = swing(0.0, (x, y), us, ends);
        let (low, high) = match meridian {
            Meridian::Straight { start, slope } => along_line((start, slope), across, n, vs),
            Meridian::Round { centre, radius } => along_circle((centre, radius), across, n, vs),
        };
        (o + low, o + high)
    };
    let (at, x, y, n) = (axes.origin, axes.x, axes.y, axes.normal.get());
    let [(lx, hx), (ly, hy), (lz, hz)] =
        [extent(at.x, x.x, y.x, n.x), extent(at.y, x.y, y.y, n.y), extent(at.z, x.z, y.z, n.z)];
    let reach = match meridian {
        Meridian::Straight { start, slope } => {
            start.abs() + larger(vs.0.abs(), vs.1.abs()) * (slope.0.abs() + slope.1.abs())
        }
        Meridian::Round { centre, radius } => centre.abs() + radius,
    };
    // The owned sin_cos and atan2 are within an ulp, and every sum here a few roundings: thirty-two machine epsilons of
    // the largest magnitude in play covers them all.
    let margin = Length::from_millimetres(32.0 * f64::EPSILON * (at.x.abs() + at.y.abs() + at.z.abs() + reach));
    let corner = |x: f64, y: f64, z: f64| Point::<World>::from_vec3(Vec3::new(x, y, z));
    Bounds::around(corner(lx, ly, lz), margin).union(&Bounds::around(corner(hx, hy, hz), margin))
}

/// The least and greatest of `ρ(v) e + ζ(v) n` for `e` from `across.0` to `across.1` and `v` from `vs.0` to `vs.1`, on
/// a line `(start, slope)`. For each `v` it is linear in `e`, so its greatest over `e` is the larger of two functions
/// linear in `v`, which is convex, and its least the smaller, which is concave: both are extreme at an end of `vs`.
fn along_line((start, slope): (f64, (f64, f64)), across: (f64, f64), n: f64, vs: (f64, f64)) -> (f64, f64) {
    let mut extremes = (f64::INFINITY, f64::NEG_INFINITY);
    for v in [vs.0, vs.1] {
        let (rho, zeta) = (start + v * slope.0, v * slope.1);
        for e in [across.0, across.1] {
            let value = rho * e + zeta * n;
            extremes = (smaller(extremes.0, value), larger(extremes.1, value));
        }
    }
    extremes
}

/// The least and greatest of `ρ(v) e + ζ(v) n` as [`along_line`], on a circle `(centre, radius)`. Where `ρ` is not
/// below zero over `vs`, the greatest is at `e = across.1`, where it is `centre e + radius (e cos v + n sin v)`, whose
/// extremes in `v` are a [`swing`]; the least likewise at `across.0`. Elsewhere, outside a sphere's domain, it is the
/// whole circle's reach.
fn along_circle((centre, radius): (f64, f64), across: (f64, f64), n: f64, vs: (f64, f64)) -> (f64, f64) {
    let (start, end) = (sin_cos(vs.0), sin_cos(vs.1));
    let least_rho = swing(centre, (radius, 0.0), vs, (centre + radius * start.1, centre + radius * end.1)).0;
    if least_rho < 0.0 {
        let reach = centre.abs() + radius;
        return (-reach, reach);
    }
    let over_v = |e: f64| {
        let ends = (e * start.1 + n * start.0, e * end.1 + n * end.0);
        let (low, high) = swing(0.0, (e, n), vs, ends);
        (centre * e + radius * low, centre * e + radius * high)
    };
    (over_v(across.0).0, over_v(across.1).1)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use occam_units::{Local, Placement, Vector};

    use super::*;

    /// The axes of a placement turned not at all and moved to `origin`.
    fn moved(origin: Vec3) -> Axes {
        let shift = Placement::<World, World>::translation(Vector::from_vec3(origin));
        Axes::of(&Placement::<Local, World>::identity().then(&shift))
    }

    /// How far `b`'s lowest x and z reach below 0, where the margin can be read to a part in a thousand.
    fn below_zero(b: Bounds) -> [f64; 2] {
        [-b.low().to_vec3().x, -b.low().to_vec3().z]
    }

    #[test]
    fn a_box_is_padded_by_thirty_two_epsilons_of_its_size() {
        // A cone from v = -2 to 5, moved so that its box's lowest x and z are 0: the size is the origin's coordinates
        // and the meridian's reach, its start plus the larger end times the slope's two parts.
        let (sin, cos) = sin_cos(0.5);
        let straight = Meridian::Straight { start: 1.0, slope: (sin, cos) };
        let origin = Vec3::new(1.0 + 5.0 * sin, -3.0, 2.0 * cos);
        let patch = UvBox::new(UvPoint::new(0.0, -2.0), UvPoint::new(TAU, 5.0));
        let size = origin.x + 3.0 + origin.z + (1.0 + 5.0 * (sin + cos));
        let margin = 32.0 * f64::EPSILON * size;
        for got in below_zero(turned_box(&moved(origin), straight, patch)) {
            assert!((got - margin).abs() <= 1e-3 * margin, "{got} against {margin}");
        }
        // A torus of radii 3 and 1, moved likewise: the reach is its centre's distance plus its radius.
        let round = Meridian::Round { centre: 3.0, radius: 1.0 };
        let whole = UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(TAU, TAU));
        let margin = 32.0 * f64::EPSILON * (4.0 + 3.0 + 1.0 + 4.0);
        for got in below_zero(turned_box(&moved(Vec3::new(4.0, -3.0, 1.0)), round, whole)) {
            assert!((got - margin).abs() <= 1e-3 * margin, "{got} against {margin}");
        }
    }

    #[test]
    fn a_round_box_past_where_the_meridian_crosses_the_axis_still_holds_it() {
        // A sphere's latitude past either pole, where the meridian is on the far side of the axis.
        let (axes, round) = (moved(Vec3::new(0.0, 0.0, 0.0)), Meridian::Round { centre: 0.0, radius: 2.0 });
        for (lo, hi) in [(0.0, 3.0), (-3.0, 0.0)] {
            let b = turned_box(&axes, round, UvBox::new(UvPoint::new(0.0, lo), UvPoint::new(1.0, hi)));
            for i in 0..=40 {
                for j in 0..=40 {
                    let uv = UvPoint::new(f64::from(i) / 40.0, lo + (hi - lo) * f64::from(j) / 40.0);
                    assert!(b.contains(point(&axes, round, uv)), "{uv:?}: {b:?}");
                }
            }
        }
    }
}
