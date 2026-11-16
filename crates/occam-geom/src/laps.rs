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
    /// The turns the far reading starts with.
    pub(crate) far: f64,
    /// The least and greatest angle the curve reaches.
    pub(crate) span: (f64, f64),
}

/// Where `f` changes sign between `lo` and `hi`, where it does, by bisection.
fn crossing(f: &impl Fn(f64) -> f64, (mut lo, mut hi): (f64, f64)) -> f64 {
    let rising = f(hi) >= 0.0;
    for _ in 0..80 {
        let mid = 0.5 * lo + 0.5 * hi;
        if mid <= lo || mid >= hi {
            break;
        }
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
    /// finite, with `at` the angle of its middle. `None` where it crosses the plane through the axis more than
    /// [`MOST`] times.
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
        let (east, north) = across(lo);
        let start = at + angle_from(at, east, north);
        let far = TAU * ((start - (at + PI + angle_from(at + PI, east, north))) / TAU).round();
        Some(Self { at, crossings, far, span: Self::reach(at, far, &crossings) })
    }

    /// The least and greatest angle either reading can give, with every count of turns it can have.
    fn reach(at: f64, far: f64, crossings: &[(f64, f64, f64)]) -> (f64, f64) {
        let (mut near_turns, mut far_turns) = (0.0, far);
        let (mut least, mut most) = (at - PI, at + PI);
        let mut widen = |low: f64, high: f64| {
            least = if low < least { low } else { least };
            most = if high > most { high } else { most };
        };
        widen(at + far, at + TAU + far);
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
        let (mut near_turns, mut far_turns) = (R::splat(0.0), R::splat(self.far));
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

    /// The numbers that define them.
    pub(crate) fn numbers(&self) -> Vec<f64> {
        let mut out = vec![self.at, self.far];
        out.extend(self.crossings.iter().flat_map(|(t, n, f)| [*t, *n, *f]));
        out
    }
}

/// A vector's parts along two axes.
pub(crate) fn across(q: Vec3, (x, y): (Vec3, Vec3)) -> (f64, f64) {
    (q.dot(x), q.dot(y))
}
