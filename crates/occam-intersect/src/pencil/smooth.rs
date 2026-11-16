//! Source: levin-1979
//! The smooth quartic (Levin 1979): `v = (−β ± √Δ)/α` on the rulings where `Δ ≥ 0`, a loop round the whole turn for
//! each sign where `Δ` has no root, and else a loop over each arc between two roots where it is positive, both signs
//! joined at the roots.
//!
//! Where the ruled surface is a cone some of whose rulings run along the other's asymptotic cone, `α` vanishes there
//! and one sign runs to infinity. Each loop is then given within the extent asked: where its ruling parameter, its
//! distance from the apex, is no larger than a reach `V` beyond which no point of the cone is in the extent. The loop's
//! ruling parameter is `±V` where `α V² ± 2β V + γ` vanishes, at the angles where `(α V² + γ)² − 4 V² β²` does, each of
//! which the loop passes once or, between two roots of `Δ`, twice, once with each sign.

use std::f64::consts::TAU;

use occam_geom::{Curve, Quartic};
use occam_math::{atan2, sin_cos};
use occam_units::Vec3;

use super::Pair;
use super::fan::{Ball, stretches};
use super::quadric::{Along, Kind, Ruling};
use super::trig::Trig;

/// How a loop's parameter `s` gives the angle `θ` of its ruling.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Angle {
    /// `θ = s`.
    Whole,
    /// `θ = m + φ`, where `sin(φ/2) = k cos s`.
    Between { middle: f64, k: f64 },
}

impl Angle {
    /// The parameters from 0 to a whole turn at which the loop is on the ruling at `theta`.
    fn at(self, theta: f64) -> Vec<f64> {
        match self {
            Self::Whole => vec![theta.rem_euclid(TAU)],
            Self::Between { middle, k } => {
                // `sin(φ/2) = sin φ / 2 cos(φ/2)`, with `2 cos(φ/2) = √(2 (1 + cos φ))` for `φ = θ − m` within a half
                // turn: from φ's sine and cosine, which need no whole turn taken out.
                let (sin, cos) = sin_cos(theta - middle);
                let c = sin / (2.0 * (1.0 + cos)).sqrt() / k;
                if c.is_nan() || c.abs() > 1.0 {
                    return Vec::new();
                }
                let s = atan2((1.0 - c * c).sqrt(), c);
                vec![s, (TAU - s).rem_euclid(TAU)]
            }
        }
    }
}

/// A loop of the quartic, and how its parameter gives the angle of its ruling.
pub(super) struct Loop {
    pub(super) quartic: Quartic,
    angle: Angle,
}

/// `c` with zeros after it, to five coefficients.
fn five(c: &Trig) -> [f64; 5] {
    std::array::from_fn(|i| c.0.get(i).copied().unwrap_or(0.0))
}

/// `Δ / F` for `F = cos(θ − m) − cos h`, which divides it: `E`, of degree one.
fn quotient(delta: &[f64; 5], (cos_h, cos_m, sin_m): (f64, f64, f64)) -> [f64; 3] {
    let [_, d1c, d1s, d2c, d2s] = *delta;
    let f0 = -cos_h;
    let e1 = 2.0 * (cos_m * d2c + sin_m * d2s);
    let e2 = 2.0 * (cos_m * d2s - sin_m * d2c);
    let e0 = cos_m * (d1c - f0 * e1) + sin_m * (d1s - f0 * e2);
    [e0, e1, e2]
}

/// A loop's numbers before the way its root is taken: the ruled surface's axes in the world, what it is, `α` and `β`.
fn head(pair: &Pair, ruling: &Ruling, along: &Along) -> Vec<f64> {
    let origin = pair.world(ruling.origin);
    let mut out: Vec<f64> = [origin, ruling.x, ruling.y, ruling.z].iter().flat_map(|v| [v.x, v.y, v.z]).collect();
    out.extend(match ruling.kind {
        Kind::Upright { radius } => [0.0, radius, 0.0],
        Kind::Slanted { sin, cos } => [1.0, sin, cos],
    });
    out.extend(five(&along.alpha));
    out.extend((0..3).map(|i| along.beta.0.get(i).copied().unwrap_or(0.0)));
    out
}

/// The loops of a smooth quartic on `ruling`: `None` where one cannot be made of its numbers.
pub(super) fn loops(pair: &Pair, ruling: &Ruling, along: &Along) -> Option<Vec<Loop>> {
    let delta = five(&along.delta);
    let roots = along.delta.roots();
    let make = |root: &[f64], angle: Angle| -> Option<Loop> {
        let numbers: Vec<f64> =
            head(pair, ruling, along).into_iter().chain(root.iter().copied()).chain([0.0, TAU, 2.0]).collect();
        Some(Loop { quartic: Quartic::restore(&numbers).ok()?, angle })
    };
    if roots.is_empty() {
        if along.delta.at(0.0) <= 0.0 {
            return Some(Vec::new());
        }
        let signs = [1.0, -1.0].map(|sign| make(&[vec![0.0], delta.to_vec(), vec![sign]].concat(), Angle::Whole));
        return signs.into_iter().collect();
    }
    let ends = roots.iter().copied().zip(roots.iter().skip(1).copied().chain(roots.first().map(|r| r + TAU)));
    ends.filter(|(from, to)| along.delta.at(0.5 * from + 0.5 * to) > 0.0)
        .map(|(from, to)| {
            let (middle, half) = (0.5 * from + 0.5 * to, 0.5 * (to - from));
            let ((sin_m, cos_m), (_, cos_h), (k, _)) = (sin_cos(middle), sin_cos(half), sin_cos(0.5 * half));
            let e = quotient(&delta, (cos_h, cos_m, sin_m));
            make(&[1.0, cos_m, sin_m, k, e[0], e[1], e[2]], Angle::Between { middle, k })
        })
        .collect()
}

/// The stretches of the parameter of `whole`, a loop on the rulings of a cone, `ruling`, in the pair's frame about
/// `origin`, where its distance from the apex is within the reach of `ball`: its distance from the ball's centre plus
/// the ball's radius, beyond which no point of the cone is inside. Each stretch runs from one place where the loop is
/// that far to the next, round the loop. Only a cone's loops reach infinity: a cylinder's `α` is a constant.
pub(super) fn within(whole: &Loop, (ruling, along): (&Ruling, &Along), origin: Vec3, ball: Ball) -> Vec<(f64, f64)> {
    let far = ball.radius + (ruling.origin - ball.centre).norm();
    let side = along.alpha.scaled(far * far).plus(&along.gamma);
    let edge = side.times(&side).plus(&along.beta.times(&along.beta).scaled(-4.0 * far * far));
    let mut cuts: Vec<f64> = edge.roots().into_iter().flat_map(|theta| whole.angle.at(theta)).collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| a.to_bits() == b.to_bits());
    stretches(&cuts, |s| (whole.quartic.point(s).to_vec3() - origin - ruling.origin).norm() <= far)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn a_loop_is_on_a_ruling_once_round_the_whole_turn_and_twice_or_never_between_two_roots() {
        assert_eq!(Angle::Whole.at(1.2), vec![1.2]);
        assert_eq!(Angle::Whole.at(1.2 - TAU), vec![(1.2 - TAU).rem_euclid(TAU)]);
        let k = sin_cos(0.25).0;
        let between = Angle::Between { middle: 1.0, k };
        for theta in [1.2, 1.2 + 3.0 * TAU, 0.6, 1.0, 1.49] {
            let got = between.at(theta);
            assert_eq!(got.len(), 2, "{theta}: {got:?}");
            for s in &got {
                let phi = (theta - 1.0 + PI).rem_euclid(TAU) - PI;
                let off = sin_cos(0.5 * phi).0 - k * sin_cos(*s).1;
                assert!(off.abs() <= 1e-15 && (0.0..TAU).contains(s), "{theta}: {s} is {off} off");
            }
            assert!(sin_cos(got[0]).0 >= 0.0 && sin_cos(got[1]).0 <= 0.0, "{theta}: one with each sign: {got:?}");
        }
        assert_eq!(between.at(1.6), Vec::<f64>::new(), "past the half-width of 0.5");
        assert_eq!(between.at(0.4), Vec::<f64>::new());
        assert_eq!(between.at(1.0 + PI), Vec::<f64>::new(), "half a turn on");
        // At the end of its arc, where `sin(φ/2) = k` to the last bit, the loop is on the ruling once, at `s = 0`.
        let (sin, cos) = sin_cos(0.5);
        let end = Angle::Between { middle: 1.0, k: sin / (2.0 * (1.0 + cos)).sqrt() };
        assert_eq!(end.at(1.5), vec![0.0, 0.0]);
    }

    #[test]
    fn a_loop_is_cut_where_it_is_as_far_from_the_apex_as_the_ball_reaches() {
        let (s, c) = sin_cos(0.7);
        let pair = super::super::tests::unplaced(
            super::super::tests::opening(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 0.5),
            super::super::tests::opening(Vec3::new(2.0, 0.0, 0.0), Vec3::new(s, 0.0, c), 0.5),
        );
        let Some((_, o, ruling)) = pair.ruled() else { panic!("no ruled member") };
        let along = ruling.along(&pair.quadric(o));
        let apex = pair.world(ruling.origin);
        // A ball about `(−3, 1, 0.5)`, away from the apex, so that how far it reaches along a ruling is its radius and
        // more.
        let (centre, radius) = (Vec3::new(-3.0, 1.0, 0.5), 4.0);
        let ball = Ball { centre: centre - pair.origin, radius };
        let far = radius + (apex - centre).norm();
        let mut ends = 0;
        for whole in &loops(&pair, &ruling, &along).unwrap() {
            for (lo, hi) in within(whole, (&ruling, &along), pair.origin, ball) {
                if hi - lo >= TAU {
                    continue;
                }
                for t in [lo, hi] {
                    let got = (whole.quartic.point(t).to_vec3() - apex).norm();
                    assert!((got - far).abs() <= 1e-9 * far, "an end {got} from the apex, against {far}");
                    ends += 1;
                }
            }
        }
        assert!(ends >= 2, "{ends} ends");
    }
}
