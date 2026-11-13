//! Source: original
//! What the circle and the ellipse share: a parameter that is an angle and comes round, derivatives that turn by a
//! quarter at each order, and the box of an arc.

use std::f64::consts::TAU;

use occam_math::{Real, atan2, sin_cos};
use occam_units::{Bounds, Length, ParamKind, Point, Tangent, Vec3, World};

use crate::axes::Axes;
use crate::domain::{ParamRange, Wrap};
use crate::support::everything;

/// A whole turn, from 0 to 2π, coming round.
pub(crate) const fn turn() -> ParamRange {
    ParamRange { lo: 0.0, hi: TAU, wrap: Wrap::Periodic, kind: ParamKind::Angle }
}

/// `(cos, sin)` of `t` plus `k` quarter turns, given `(cos t, sin t)`: the coefficients of the `k`-th derivative.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn quarter_turns<R: Real>(cos: R, sin: R, k: u8) -> (R, R) {
    match k % 4 {
        0 => (cos, sin),
        1 => (-sin, cos),
        2 => (-cos, -sin),
        _ => (sin, -cos),
    }
}

/// `origin + a cos t x + b sin t y`.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn on_turn<R: Real>(axes: &Axes, (a, b): (f64, f64), t: R) -> Point<World, R> {
    let (sin, cos) = sin_cos(t);
    axes.at(R::splat(a) * cos, R::splat(b) * sin)
}

/// [`on_turn`] and its derivative.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn on_turn_first<R: Real>(axes: &Axes, (a, b): (f64, f64), t: R) -> (Point<World, R>, Tangent<World, R>) {
    let (sin, cos) = sin_cos(t);
    let (a, b) = (R::splat(a), R::splat(b));
    (axes.at(a * cos, b * sin), Tangent::new(axes.along(-(a * sin), b * cos)))
}

/// [`on_turn`] and its first and second derivatives, the second `-(a cos t x + b sin t y)`, as
/// [`turn_derivatives`] gives it.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn on_turn_second<R: Real>(
    axes: &Axes,
    radii: (f64, f64),
    t: R,
) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
    let (point, rate) = on_turn_first(axes, radii, t);
    let (sin, cos) = sin_cos(t);
    let (a, b) = (R::splat(radii.0), R::splat(radii.1));
    (point, rate, Tangent::new(axes.along(a * -cos, b * -sin)))
}

/// Appends [`on_turn`]'s derivatives of order 1 to `order`.
pub(crate) fn turn_derivatives(axes: &Axes, (a, b): (f64, f64), t: f64, order: u8, out: &mut Vec<Tangent<World>>) {
    let (sin, cos) = sin_cos(t);
    for k in 1..=order {
        let (along_x, along_y) = quarter_turns(cos, sin, k);
        out.push(Tangent::new(axes.along(a * along_x, b * along_y)));
    }
}

/// `angle` moved by whole turns to within one turn above `lo`.
pub(crate) fn above(angle: f64, lo: f64) -> f64 {
    let moved = angle - ((angle - lo) / TAU).floor() * TAU;
    if moved < lo { lo } else { moved }
}

/// The smaller of two numbers, the first if they are equal.
pub(crate) fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The larger of two numbers, the first if they are equal.
pub(crate) fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The least and the greatest of `o + x cos t + y sin t` for `t` from `lo` to `hi`, both finite, whose values at the ends
/// are `ends`. It is `o + A cos(t − φ)`, so it reaches `o + A` where the range holds `φ`, the angle of `(x, y)`, and
/// `o − A` where it holds the angle of `(−x, −y)`, and is otherwise extreme at an end. That angle is found as itself,
/// not as `φ` and half a turn, which `above` would take the same whether the half turn were added or taken away.
pub(crate) fn swing(o: f64, (x, y): (f64, f64), (lo, hi): (f64, f64), ends: (f64, f64)) -> (f64, f64) {
    let whole = hi - lo >= TAU;
    let holds = |angle: f64| whole || above(angle, lo) <= hi;
    let amplitude = (x * x + y * y).sqrt();
    let low = if holds(atan2(-y, -x)) { o - amplitude } else { smaller(ends.0, ends.1) };
    let high = if holds(atan2(y, x)) { o + amplitude } else { larger(ends.0, ends.1) };
    (low, high)
}

/// The box of `origin + a cos t x + b sin t y` for `t` from `lo` to `hi`, rounding included: each coordinate's
/// [`swing`], with the ends `from` and `to`.
pub(crate) fn arc_box(axes: &Axes, radii: (f64, f64), (lo, hi): (f64, f64), (from, to): (Vec3, Vec3)) -> Bounds {
    if !(lo.is_finite() && hi.is_finite()) {
        return everything();
    }
    let extremes = |o: f64, x: f64, y: f64, ends: (f64, f64)| swing(o, (radii.0 * x, radii.1 * y), (lo, hi), ends);
    let (at, first, second) = (axes.origin, axes.x, axes.y);
    let [(lx, hx), (ly, hy), (lz, hz)] = [
        extremes(at.x, first.x, second.x, (from.x, to.x)),
        extremes(at.y, first.y, second.y, (from.y, to.y)),
        extremes(at.z, first.z, second.z, (from.z, to.z)),
    ];
    // The owned sin_cos is within an ulp, and every sum here a few roundings: sixteen machine epsilons of the largest
    // magnitude in play covers them all.
    let size = at.x.abs() + at.y.abs() + at.z.abs() + radii.0 + radii.1;
    let margin = Length::from_millimetres(16.0 * f64::EPSILON * size);
    let corner = |x: f64, y: f64, z: f64| Point::<World>::from_vec3(Vec3::new(x, y, z));
    Bounds::around(corner(lx, ly, lz), margin).union(&Bounds::around(corner(hx, hy, hz), margin))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use occam_units::{Local, Placement, Vector};

    use super::*;

    #[test]
    fn the_least_is_where_the_range_holds_the_opposite_direction() {
        // 3 + cos t + sin t is 3 + √2 cos(t − π/4): least at 5π/4, the angle of (−1, −1), which the range from π to
        // 3π/2 holds, and greatest at π/4, which it does not, so the greater of its ends, both 2, is.
        let (lo, hi) = (PI, 1.5 * PI);
        assert_eq!(swing(3.0, (1.0, 1.0), (lo, hi), (2.0, 2.0)), (3.0 - 2.0_f64.sqrt(), 2.0));
        // Turned a quarter, 3 − cos t + sin t: greatest at 3π/4 and least at 7π/4, the angle of (1, −1), neither of which
        // the range holds, so its ends are both: 4 at π and 2 at 3π/2.
        assert_eq!(swing(3.0, (-1.0, 1.0), (lo, hi), (4.0, 2.0)), (2.0, 4.0));
    }

    #[test]
    fn an_angle_whole_turns_from_the_start_comes_back_to_it_and_never_below() {
        // A double below 17 and -19 turns: the quotient rounds to the whole number, and the turns taken away overshoot.
        assert_eq!([above((17.0 * TAU).next_down(), 0.0), above((-19.0 * TAU).next_down(), 0.0)], [0.0; 2]);
        assert_eq!([above(-1.0, 0.0), above(1.0, 0.0), above(9.0, 2.0)], [TAU - 1.0, 1.0, 9.0 - TAU]);
    }

    #[test]
    fn an_arc_box_is_padded_by_sixteen_epsilons_of_its_size() {
        // An ellipse about (1, -3, 7) with radii 1 and 3, so that its box's lowest x and highest y are 0, where the
        // margin can be read to a part in a thousand: the size is 1 + 3 + 7 + 1 + 3.
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(1.0, -3.0, 7.0)));
        let axes = Axes::of(&Placement::<Local, World>::identity().then(&shift));
        let ends = (Vec3::new(2.0, -3.0, 7.0), Vec3::new(2.0, -3.0, 7.0));
        let b = arc_box(&axes, (1.0, 3.0), (0.0, TAU), ends);
        let margin = 16.0 * f64::EPSILON * 15.0;
        for got in [-b.low().to_vec3().x, b.high().to_vec3().y] {
            assert!((got - margin).abs() <= 1e-3 * margin, "{got} against {margin}: {b:?}");
        }
    }
}
