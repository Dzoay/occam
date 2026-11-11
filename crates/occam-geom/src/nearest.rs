//! Source: original
//! The closest point of a curve to a point where no formula gives it: the search range cut into pieces, each piece where
//! the distance turns from falling to rising refined by Newton's method kept inside it, and the nearest of those and the
//! ends. It needs nothing of the family but its derivatives, so every family without a closed form shares it.

use occam_units::{Point, Tangent, Vec3, World};

use crate::batch::Scratch;
use crate::curve::ParametricCurve;

/// How many pieces the search range is cut into.
const PIECES: u32 = 128;

/// The most steps that refine one piece. Each keeps the root bracketed and at least halves the bracket when Newton's
/// step would leave it, so a piece is refined to adjacent doubles well within this, and it stops sooner where Newton's
/// step no longer moves it.
const STEPS: u32 = 200;

/// Half the derivative of the squared distance from `q` at `t`, `(c − q)·c'`, and its derivative, `|c'|² + (c − q)·c''`.
fn slope<C: ParametricCurve>(curve: &C, q: Vec3, t: f64, scratch: &mut Scratch) -> (f64, f64) {
    let mut rates: Vec<Tangent<World>> = Vec::with_capacity(2);
    let (point, first) = curve.eval_first(t);
    let second = match curve.derivatives(t, 2, &mut rates, scratch) {
        Ok(()) => rates.get(1).map_or(Vec3::new(f64::NAN, f64::NAN, f64::NAN), |d| d.rate()),
        Err(_) => Vec3::new(f64::NAN, f64::NAN, f64::NAN),
    };
    let (off, first) = (point.to_vec3() - q, first.rate());
    (off.dot(first), first.dot(first) + off.dot(second))
}

/// The root of [`slope`] between `a`, where it is below zero, and `b`, where it is not, and how many steps found it.
fn refine<C: ParametricCurve>(
    curve: &C,
    q: Vec3,
    (mut low, mut high): (f64, f64),
    scratch: &mut Scratch,
) -> (f64, u32) {
    let mut at = 0.5 * low + 0.5 * high;
    for step in 1..=STEPS {
        let (rate, change) = slope(curve, q, at, scratch);
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

/// The parameter from `lo` to `hi`, both finite, of the point of `curve` nearest `p`. Of two exactly as near, the
/// first found from `lo`.
pub(crate) fn nearest<C: ParametricCurve>(
    curve: &C,
    p: Point<World>,
    (lo, hi): (f64, f64),
    scratch: &mut Scratch,
) -> f64 {
    let q = p.to_vec3();
    let distance = |t: f64| (curve.eval_point(t).to_vec3() - q).norm();
    let mut best = (lo, distance(lo));
    let mut consider = |t: f64| {
        let d = distance(t);
        if d < best.1 {
            best = (t, d);
        }
    };
    consider(hi);
    let step = (hi - lo) / f64::from(PIECES);
    let mut before = (lo, slope(curve, q, lo, scratch).0);
    for piece in 1..=PIECES {
        let t = if piece == PIECES { hi } else { lo + step * f64::from(piece) };
        let g = slope(curve, q, t, scratch).0;
        if before.1 < 0.0 && g >= 0.0 {
            consider(refine(curve, q, (before.0, t), scratch).0);
        }
        before = (t, g);
    }
    best.0
}

#[cfg(test)]
mod tests {
    use occam_math::atan2;
    use occam_units::{Length, Placement};

    use super::*;
    use crate::Circle;

    #[test]
    fn newtons_step_does_the_work() {
        // The foot of (13, 4, 2) on a circle of radius 10 is at atan2(4, 13). From a bracket a twentieth wide,
        // bisection alone would take some fifty steps to reach adjacent doubles; Newton's method takes a handful.
        let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(10.0)).unwrap();
        let foot = atan2(4.0, 13.0);
        let (t, steps) =
            refine(&circle, Vec3::new(13.0, 4.0, 2.0), (foot - 0.03, foot + 0.02), &mut Scratch::default());
        assert!((t - foot).abs() < 1e-15 && steps <= 8, "{t} in {steps} steps");
    }

    #[test]
    fn a_newton_step_that_would_leave_the_bracket_is_not_taken() {
        // The slope there is 136 sin(t − foot): from the middle of this bracket, where it is nearly flat, Newton's step
        // lands some nine radians below it, by the far side of the circle, three half-turns away.
        let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(10.0)).unwrap();
        let foot = atan2(4.0, 13.0);
        let (t, _) = refine(&circle, Vec3::new(13.0, 4.0, 2.0), (foot - 0.05, foot + 3.0), &mut Scratch::default());
        assert!((t - foot).abs() < 1e-15, "{t}");
    }
}
