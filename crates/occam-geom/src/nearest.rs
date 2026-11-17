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

/// The ends of the pieces a search cuts a range of a curve into, each with the curve's point and first two derivatives
/// there. Read once, they serve every point the curve is searched for the foot of, as joining an edge to its face
/// searches each curve for the feet of the other's samples.
pub(crate) struct Grid {
    at: Vec<(f64, [Vec3; 3])>,
}

impl Grid {
    /// The ends of the [`PIECES`] pieces from `lo` to `hi`, both finite, `lo` and `hi` themselves at the ends, each with
    /// the curve's point and first two derivatives there as `jet` gives them.
    pub(crate) fn of(jet: &impl Fn(f64) -> [Vec3; 3], (lo, hi): (f64, f64)) -> Self {
        let step = (hi - lo) / f64::from(PIECES);
        let mut at = vec![(lo, jet(lo))];
        at.extend((1..=PIECES).map(|piece| {
            let t = if piece == PIECES { hi } else { lo + step * f64::from(piece) };
            (t, jet(t))
        }));
        Self { at }
    }
}

/// The parameter, in the range `grid` was read over, of the point nearest `q` of the curve whose point and first two
/// derivatives at each parameter `jet` gives, as `grid` holds them at the ends of its pieces. Of two exactly as near,
/// the first found from the range's low end.
pub(crate) fn nearest_in(grid: &Grid, jet: &impl Fn(f64) -> [Vec3; 3], q: Vec3) -> f64 {
    let distance = |point: Vec3| (point - q).norm();
    let rate = |&(_, [point, first, _]): &(f64, [Vec3; 3])| (point - q).dot(first);
    let Some(&(lo, [low, ..])) = grid.at.first() else { return f64::NAN };
    let mut best = (lo, distance(low));
    let mut consider = |t: f64, d: f64| {
        if d < best.1 {
            best = (t, d);
        }
    };
    if let Some(&(hi, [high, ..])) = grid.at.last() {
        consider(hi, distance(high));
    }
    for pair in grid.at.windows(2) {
        if let [before, after] = pair
            && rate(before) < 0.0
            && rate(after) >= 0.0
        {
            let t = refine(jet, q, (before.0, after.0)).0;
            consider(t, distance(jet(t)[0]));
        }
    }
    best.0
}

/// The parameter from `lo` to `hi`, both finite, of the point nearest `q` of the curve whose point and first two
/// derivatives at each parameter `jet` gives. Of two exactly as near, the first found from `lo`.
pub(crate) fn nearest_by(jet: impl Fn(f64) -> [Vec3; 3], q: Vec3, range: (f64, f64)) -> f64 {
    nearest_in(&Grid::of(&jet, range), &jet, q)
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
    fn a_grid_read_once_serves_every_search_of_its_curve_and_each_finds_the_foot_a_search_alone_finds() {
        // A circle of radius 10 read at the ends of its pieces once: each foot reads it again only to refine, is where
        // the point's direction from the centre meets the circle, or the range's nearer end, and is the one a search of
        // its own finds, to the bit.
        let reads = core::cell::Cell::new(0_u32);
        let round = circle();
        let counted = |t: f64| {
            reads.set(reads.get().saturating_add(1));
            round(t)
        };
        let grid = Grid::of(&counted, (0.5, 6.0));
        assert_eq!(reads.replace(0), PIECES.saturating_add(1), "each end of each piece once");
        let towards = |angle: f64, out: f64, up: f64| {
            let (sin, cos) = occam_math::sin_cos(angle);
            Vec3::new(out * cos, out * sin, up)
        };
        let feet = [
            (towards(1.0, 13.0, 2.0), 1.0),
            (towards(2.5, 4.0, -1.0), 2.5),
            (towards(4.0, 30.0, 0.0), 4.0),
            (towards(5.9, 9.0, 3.0), 5.9),
            (towards(0.3, 12.0, 0.0), 0.5),
            (towards(6.2, 12.0, 0.0), 6.0),
        ];
        for (q, foot) in feet {
            let found = nearest_in(&grid, &counted, q);
            assert!(reads.replace(0) < PIECES, "{q:?}: only refining reads it again");
            assert!((found - foot).abs() < 1e-12, "{q:?}: {found}, not {foot}");
            assert_eq!(found.to_bits(), nearest_by(circle(), q, (0.5, 6.0)).to_bits(), "{q:?}");
        }
    }

    #[test]
    fn a_grid_is_read_at_its_range_s_ends_and_at_even_steps_between() {
        let grid = Grid::of(&circle(), (0.5, 6.0));
        let at: Vec<f64> = grid.at.iter().map(|&(t, _)| t).collect();
        let step = 5.5 / f64::from(PIECES);
        let expected: Vec<f64> = (0..=PIECES)
            .map(|piece| match piece {
                0 => 0.5,
                PIECES => 6.0,
                _ => 0.5 + step * f64::from(piece),
            })
            .collect();
        assert_eq!(at, expected);
        let jets = grid.at.iter().all(|&(t, jet)| jet == circle()(t));
        assert!(jets, "each with the curve's point and derivatives there");
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
