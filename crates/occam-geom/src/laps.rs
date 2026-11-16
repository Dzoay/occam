//! Source: original
//! [`Laps`]: how an image keeps the angle about a chart's axis continuous for a curve that may go round the axis any
//! number of times, but not through it. Two readings of the angle are kept, one measured from the direction `at` and
//! one from its opposite, and each jumps by a turn only where the curve crosses the half-plane behind it. The turns
//! each has gained by a parameter are counted from where the curve crosses those half-planes, found once when the image
//! is made, and the reading used is the one whose jumps are a quarter turn or more away. So the angle is a closed form,
//! `atan2` and whole turns, exact wherever the curve is not on the axis.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use occam_math::{Real, sin_cos};
use occam_units::Vec3;

use crate::image::angle_from;

/// The most crossings of the plane through the axis kept: a plane meets a quartic in four points, and a part of one,
/// cut where it passes a pole, in no more.
const MOST: usize = 8;

/// The samples a curve's range is searched in for crossings.
const SAMPLES: u32 = 512;

/// The crossings of the plane through the axis at the angle `at`, and what they add to each reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Laps {
    /// The direction the near reading is measured from.
    pub(crate) at: f64,
    /// Where the curve crosses the plane, and the turns added after it to the near reading and to the far one. Unused
    /// slots cross at infinity.
    pub(crate) crossings: [(f64, f64, f64); MOST],
    /// The least and greatest angle the curve reaches.
    pub(crate) span: (f64, f64),
}

/// The turns the far reading starts with: the curve starts a quarter turn before `at`, where the near reading is
/// `at − π/2` and the far one, from `at + π`, is a quarter turn on from it, `at + 3π/2`, a turn more.
const FAR: f64 = -TAU;

/// Where `f` changes sign between `lo` and `hi`, where it does, by bisection.
fn crossing(f: &impl Fn(f64) -> f64, (mut lo, mut hi): (f64, f64)) -> f64 {
    let rising = f(hi) >= 0.0;
    for _ in 0..80 {
        let mid = 0.5 * lo + 0.5 * hi;
        if (f(mid) >= 0.0) == rising {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * lo + 0.5 * hi
}

impl Laps {
    /// The laps of the curve whose point across the axis, in the chart's frame, `across` gives from `lo` to `hi`, both
    /// finite, with `at` a quarter turn on from the angle of its start, which its angle therefore starts at. `None`
    /// where it crosses the plane through the axis more than [`MOST`] times.
    pub(crate) fn counted(across: impl Fn(f64) -> (f64, f64), (lo, hi): (f64, f64), at: f64) -> Option<Self> {
        let (sin, cos) = sin_cos(at);
        let normal = |t: f64| {
            let (east, north) = across(t);
            north * cos - east * sin
        };
        let ahead = |t: f64| {
            let (east, north) = across(t);
            east * cos + north * sin
        };
        let mut crossings = [(f64::INFINITY, 0.0, 0.0); MOST];
        let mut found = 0_usize;
        let step = (hi - lo) / f64::from(SAMPLES);
        let mut before = (lo, normal(lo));
        for i in 1..=SAMPLES {
            let t = if i == SAMPLES { hi } else { lo + step * f64::from(i) };
            let now = (t, normal(t));
            if (before.1 >= 0.0) != (now.1 >= 0.0) {
                let when = crossing(&normal, (before.0, now.0));
                let up = if now.1 >= 0.0 { TAU } else { -TAU };
                let turns = if ahead(when) > 0.0 { (when, 0.0, up) } else { (when, -up, 0.0) };
                *crossings.get_mut(found)? = turns;
                found = found.saturating_add(1);
            }
            before = now;
        }
        Some(Self { at, crossings, span: Self::reach(at, &crossings) })
    }

    /// The least and greatest angle either reading can give, with every count of turns it can have: at the start, from
    /// the far reading's least, `at − 2π`, to the near one's greatest, `at + π`.
    fn reach(at: f64, crossings: &[(f64, f64, f64)]) -> (f64, f64) {
        let (mut near_turns, mut far_turns) = (0.0, FAR);
        let (mut least, mut most) = (at + FAR, at + PI);
        let mut widen = |low: f64, high: f64| {
            least = if low < least { low } else { least };
            most = if high > most { high } else { most };
        };
        for (_, near, far) in crossings.iter().filter(|c| c.0.is_finite()) {
            near_turns += near;
            far_turns += far;
            widen(at - PI + near_turns, at + PI + near_turns);
            widen(at + far_turns, at + TAU + far_turns);
        }
        (least, most)
    }

    /// The continuous angle at `t` of the point `(east, north)` across the axis.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    pub(crate) fn angle<R: Real>(&self, t: R, east: R, north: R) -> R {
        let (at, back) = (R::splat(self.at), R::splat(self.at + PI));
        let (near, far) = (angle_from(at, east, north), angle_from(back, east, north));
        let (mut near_turns, mut far_turns) = (R::splat(0.0), R::splat(FAR));
        for (when, n, f) in self.crossings {
            let past = R::splat(when).lt(t);
            near_turns = near_turns + R::select(past, R::splat(n), R::splat(0.0));
            far_turns = far_turns + R::select(past, R::splat(f), R::splat(0.0));
        }
        R::select(near.abs().le(R::splat(FRAC_PI_2)), at + near + near_turns, back + far + far_turns)
    }

    /// The turns the curve makes about the axis from one end to the other: the near reading's, counted where it
    /// crosses the half-plane behind it.
    pub(crate) fn winding(&self) -> f64 {
        self.crossings.iter().filter(|c| c.0.is_finite()).map(|c| c.1).sum()
    }
}

/// A vector's parts along two axes.
pub(crate) fn across(q: Vec3, (x, y): (Vec3, Vec3)) -> (f64, f64) {
    (q.dot(x), q.dot(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The laps of the circle about the axis from the angle `from` to `to`, its parameter from 0 to 1, and the angle
    /// read along it.
    fn round(from: f64, to: f64) -> (Laps, impl Fn(f64) -> f64) {
        let across = move |u: f64| {
            let (s, c) = sin_cos(from + (to - from) * u);
            (c, s)
        };
        let laps = Laps::counted(across, (0.0, 1.0), from + FRAC_PI_2).unwrap();
        (laps, move |u: f64| {
            let (east, north) = across(u);
            laps.angle(u, east, north)
        })
    }

    #[test]
    fn a_curve_round_the_axis_keeps_its_angle_and_counts_its_turns_either_way() {
        for (to, turns) in [(0.3 + 3.0 * TAU, 3.0), (0.3 - 2.0 * TAU, -2.0), (0.5, 0.0)] {
            let (laps, angle) = round(0.3, to);
            for i in 0..=1000 {
                let u = f64::from(i) / 1000.0;
                let want = 0.3 + (to - 0.3) * u;
                assert!((angle(u) - want).abs() <= 1e-12, "to {to} at {u}: {} against {want}", angle(u));
            }
            assert_eq!(laps.winding(), turns * TAU, "to {to}");
            let at = 0.3 + FRAC_PI_2;
            let span = match turns {
                3.0 => (at - TAU, at + PI + 3.0 * TAU),
                -2.0 => (at - 3.0 * TAU, at + PI),
                _ => (at - TAU, at + PI),
            };
            assert_eq!(laps.span, span, "to {to}");
        }
    }

    #[test]
    fn a_crossing_is_found_where_its_function_rises_through_zero_or_falls() {
        assert!((crossing(&|t: f64| t - 0.3, (0.0, 1.0)) - 0.3).abs() <= 1e-15);
        assert!((crossing(&|t: f64| 0.7 - t, (0.0, 1.0)) - 0.7).abs() <= 1e-15);
        assert!((crossing(&|t: f64| t * t - 2.0, (1.0, 2.0)) - 2.0_f64.sqrt()).abs() <= 1e-15);
    }
}
