//! Source: original
//! The closest point of a curve to a point where no formula gives it: the search range cut into pieces, each piece where
//! the distance turns from falling to rising refined by Newton's method kept inside it, and the nearest of those and the
//! ends. It needs nothing but the point and its first two derivatives at each parameter, so every curve family without a
//! closed form shares it, and so does a curve derived from one, as a swept surface's foot reduces to one.

use occam_units::{Point, Vec3, World};

use crate::curve::ParametricCurve;

/// How many pieces the search range is cut into.
const PIECES: u32 = 128;

/// The most steps that refine one piece. Each keeps the root bracketed and at least halves the bracket when Newton's
/// step would leave it, so a piece is refined to adjacent doubles well within this, and it stops sooner where Newton's
/// step no longer moves it.
const STEPS: u32 = 200;

/// Half the derivative of the squared distance from `q` at `t`, `(c − q)·c'`, and its derivative, `|c'|² + (c − q)·c''`,
/// given the curve's point and first two derivatives by `jet`.
fn slope(jet: &impl Fn(f64) -> [Vec3; 3], q: Vec3, t: f64) -> (f64, f64) {
    let [point, first, second] = jet(t);
    let off = point - q;
    (off.dot(first), first.dot(first) + off.dot(second))
}

/// The root of [`slope`] between `a`, where it is below zero, and `b`, where it is not, and how many steps found it.
fn refine(jet: &impl Fn(f64) -> [Vec3; 3], q: Vec3, (mut low, mut high): (f64, f64)) -> (f64, u32) {
    let mut at = 0.5 * low + 0.5 * high;
    for step in 1..=STEPS {
        let (rate, change) = slope(jet, q, at);
        if rate < 0.0 {
            low = at;
        } else {
            high = at;
        }
        let newton = at - rate / change;
        // Where Newton's step no longer moves it, it is the root to the last bit, though it may now be an end of the
        // bracket, where the step below would bisect away from it.
        let next = if low < newton && newton < high { newton } else { 0.5 * low + 0.5 * high };
        if newton.to_bits() == at.to_bits() || next.to_bits() == at.to_bits() {
            return (at, step);
        }
        at = next;
    }
    (at, STEPS)
}

/// The parameter from `lo` to `hi`, both finite, of the point nearest `q` of the curve whose point and first two
/// derivatives at each parameter `jet` gives. Of two exactly as near, the first found from `lo`.
pub(crate) fn nearest_by(jet: impl Fn(f64) -> [Vec3; 3], q: Vec3, (lo, hi): (f64, f64)) -> f64 {
    let distance = |t: f64| {
        let [point, ..] = jet(t);
        (point - q).norm()
    };
    let mut best = (lo, distance(lo));
    let mut consider = |t: f64| {
        let d = distance(t);
        if d < best.1 {
            best = (t, d);
        }
    };
    consider(hi);
    let step = (hi - lo) / f64::from(PIECES);
    let mut before = (lo, slope(&jet, q, lo).0);
    for piece in 1..=PIECES {
        let t = if piece == PIECES { hi } else { lo + step * f64::from(piece) };
        let g = slope(&jet, q, t).0;
        if before.1 < 0.0 && g >= 0.0 {
            consider(refine(&jet, q, (before.0, t)).0);
        }
        before = (t, g);
    }
    best.0
}

/// The parameter from `lo` to `hi`, both finite, of the point of `curve` nearest `p`, from its second rung.
pub(crate) fn nearest<C: ParametricCurve>(curve: &C, p: Point<World>, range: (f64, f64)) -> f64 {
    let jet = |t: f64| {
        let (point, first, second) = curve.eval_second(t);
        [point.to_vec3(), first.rate(), second.rate()]
    };
    nearest_by(jet, p.to_vec3(), range)
}

#[cfg(test)]
mod tests {
    use occam_math::atan2;
    use occam_units::{Length, Placement};

    use super::*;
    use crate::Circle;

    fn circle() -> impl Fn(f64) -> [Vec3; 3] {
        let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(10.0)).unwrap();
        move |t| {
            let (point, first, second) = circle.eval_second(t);
            [point.to_vec3(), first.rate(), second.rate()]
        }
    }

    #[test]
    fn newtons_step_does_the_work() {
        // The foot of (13, 4, 2) on a circle of radius 10 is at atan2(4, 13). From a bracket a twentieth wide,
        // bisection alone would take some fifty steps to reach adjacent doubles; Newton's method takes a handful.
        let foot = atan2(4.0, 13.0);
        let (t, steps) = refine(&circle(), Vec3::new(13.0, 4.0, 2.0), (foot - 0.03, foot + 0.02));
        assert!((t - foot).abs() < 1e-15 && steps <= 8, "{t} in {steps} steps");
    }

    #[test]
    fn a_newton_step_that_would_leave_the_bracket_is_not_taken() {
        // The slope there is 136 sin(t − foot): from the middle of this bracket, where it is nearly flat, Newton's step
        // lands some nine radians below it, by the far side of the circle, three half-turns away.
        let foot = atan2(4.0, 13.0);
        let (t, _) = refine(&circle(), Vec3::new(13.0, 4.0, 2.0), (foot - 0.05, foot + 3.0));
        assert!((t - foot).abs() < 1e-15, "{t}");
    }

    #[test]
    fn a_foot_at_an_end_of_the_range_is_found() {
        // Just off the circle's centre towards angle 0: the nearest point is at angle 0, the range's low end.
        let foot = nearest_by(circle(), Vec3::new(0.5, 0.0, 0.0), (0.0, 6.0));
        assert!(foot.abs() < 1e-12, "{foot}");
        let inside = nearest_by(circle(), Vec3::new(0.0, 0.5, 0.0), (0.0, 6.0));
        assert!((inside - std::f64::consts::FRAC_PI_2).abs() < 1e-12, "{inside}");
    }
}
