//! Source: dupont-lazard-lazard-petitjean-2008
//! A rational curve of the pencil with one point on each line of a family (Dupont, Lazard, Lazard and Petitjean 2008,
//! part III): the rulings of a cone through a point of the other surface, as from a node or from a cone's apex on a
//! cylinder, or the rulings of a cylinder, through its vertex at infinity. Its point at the angle `θ` is `N(θ) / D(θ)`,
//! with `N` and `D` of degree two in `cos θ` and `sin θ`. So each arc of the angle, read through the rational quadratic
//! arc of its circle, is a rational Bézier curve of degree four, and a stretch of arcs is one rational B-spline, exact.
//!
//! Where `D` vanishes the curve runs to infinity, and it is given within a ball, the extent asked. It meets the ball's
//! sphere where `|N − D c|² − ρ² D²`, of degree four, vanishes, and each stretch between those roots whose middle is
//! inside is kept, as one B-spline. Its weights on each arc are `D` there, positive once the sign is taken out, and an
//! arc whose weights are not all positive is halved until they are.

use std::f64::consts::{FRAC_PI_4, TAU};

use occam_geom::{BsplineCurve, SplineCurve};
use occam_math::sin_cos;
use occam_units::{Bounds, Point, Vec3};

use super::Pair;
use super::trig::Trig;

/// A ball in the pair's frame: the extent asked, a hundredth wider, so that the roots where a curve leaves it are
/// clear of the extent's corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Ball {
    pub(super) centre: Vec3,
    pub(super) radius: f64,
}

impl Ball {
    /// The ball about the box `extent`, in the frame of `pair`.
    pub(super) fn of(pair: &Pair, extent: Bounds) -> Self {
        let (low, high) = (extent.low().to_vec3(), extent.high().to_vec3());
        let centre = (low + high) * 0.5 - pair.origin;
        Self { centre, radius: 0.505 * (high - low).norm() }
    }

    /// Whether `p`, in the pair's frame, is inside it: not where `p` is not finite.
    pub(super) fn holds(&self, p: Vec3) -> bool {
        (p - self.centre).norm() <= self.radius
    }
}

/// A curve `N(θ) / D(θ)`, in the pair's frame.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Fan {
    numerator: [Trig; 3],
    weight: Trig,
}

/// The arcs a stretch is first cut into are no wider than this: an eighth of a turn, and a hair, so that a whole turn
/// from any angle, which may round an ulp past one, is eight.
const WIDEST: f64 = FRAC_PI_4 * (1.0 + 1e-12);

/// How often an arc whose weights are not all positive is halved before the stretch is given up.
const HALVINGS: u32 = 24;

/// The most arcs one B-spline is made of, however its arcs are halved: a budget no stretch of a curve the pencil gives
/// comes near, so that one that would grow past it is given up rather than held.
const MOST: usize = 1 << 12;

/// `f g` for two quadratics in Bernstein form, in Bernstein form of degree four.
fn product([f0, f1, f2]: [f64; 3], [g0, g1, g2]: [f64; 3]) -> [f64; 5] {
    let ends = (f64::midpoint(f0 * g1, f1 * g0), f64::midpoint(f1 * g2, f2 * g1));
    [f0 * g0, ends.0, (f0 * g2 + 4.0 * f1 * g1 + f2 * g0) / 6.0, ends.1, f2 * g2]
}

/// `T W²` on the arc whose circle is `(C, S, W)` in Bernstein form, for `T` of degree two in `cos θ` and `sin θ`.
fn homogeneous(trig: &Trig, (c, s, w): ([f64; 3], [f64; 3], [f64; 3])) -> [f64; 5] {
    let k: [f64; 5] = std::array::from_fn(|i| trig.0.get(i).copied().unwrap_or(0.0));
    let (ww, cw, sw, cc, ss, cs) =
        (product(w, w), product(c, w), product(s, w), product(c, c), product(s, s), product(c, s));
    std::array::from_fn(|i| {
        let at = |b: &[f64; 5]| b.get(i).copied().unwrap_or(0.0);
        k[0] * at(&ww) + k[1] * at(&cw) + k[2] * at(&sw) + k[3] * (at(&cc) - at(&ss)) + 2.0 * k[4] * at(&cs)
    })
}

/// One arc's Bézier form: where it ends, its weights and its points, in the pair's frame.
#[derive(Clone)]
struct Arc {
    to: f64,
    weights: [f64; 5],
    points: [Vec3; 5],
}

/// `v`'s coordinates, each picked by `pick`, as constants.
fn each(pick: fn(Vec3) -> f64) -> impl Fn(&[Vec3]) -> Trig {
    move |vs| Trig(vs.iter().map(|v| pick(*v)).collect())
}

/// The three coordinates.
const AXES: [fn(Vec3) -> f64; 3] = [|v| v.x, |v| v.y, |v| v.z];

impl Fan {
    /// The curve on the lines `point + w g(θ)` of a cone through `point`, a point of the other surface, where that
    /// surface is `α w² + 2β w` along them: `w = −2β/α`, so `N = α point − 2β g` and `D = α`. `ways` are `g`'s three
    /// coefficients, its constant part and its parts along `cos θ` and `sin θ`.
    pub(super) fn through(point: Vec3, ways: [Vec3; 3], (alpha, beta): (&Trig, &Trig)) -> Self {
        let numerator = AXES.map(|pick| alpha.scaled(pick(point)).plus(&beta.times(&each(pick)(&ways)).scaled(-2.0)));
        Self { numerator, weight: alpha.clone() }
    }

    /// The curve on the lines `base(θ) + v way` of a cylinder along `way`, along which the other surface is
    /// `2β v + γ`, its vertex at infinity being on the other: `v = −γ/2β`, so `N = 2β base − γ way` and `D = 2β`.
    /// `base` is given as its three coefficients, as `ways` are in [`Fan::through`].
    pub(super) fn along(base: [Vec3; 3], way: Vec3, (beta, gamma): (&Trig, &Trig)) -> Self {
        let twice = beta.scaled(2.0);
        let numerator = AXES.map(|pick| twice.times(&each(pick)(&base)).plus(&gamma.scaled(-pick(way))));
        Self { numerator, weight: twice }
    }

    /// Its point at `theta`, in the pair's frame: not finite where `D` vanishes.
    pub(super) fn point(&self, theta: f64) -> Vec3 {
        let [x, y, z] = self.numerator.clone().map(|n| n.at(theta));
        Vec3::new(x, y, z) * (1.0 / self.weight.at(theta))
    }

    /// The stretches of the angle, each from one root to the next, round the turn, where the curve is inside `ball`:
    /// the whole turn where it never meets the ball's sphere and is inside, and none where it is outside.
    pub(super) fn within(&self, ball: Ball) -> Vec<(f64, f64)> {
        let off = |i: usize| {
            let n = self.numerator.get(i).cloned().unwrap_or_else(|| Trig(Vec::new()));
            let c = [ball.centre.x, ball.centre.y, ball.centre.z].get(i).copied().unwrap_or(0.0);
            n.plus(&self.weight.scaled(-c))
        };
        let square = |t: Trig| t.times(&t);
        let gap = square(off(0)).plus(&square(off(1))).plus(&square(off(2)));
        let edge = gap.plus(&square(self.weight.clone()).scaled(-ball.radius * ball.radius));
        stretches(&edge.roots(), |theta| ball.holds(self.point(theta)))
    }

    /// The arc of the curve from `from` to `to`, with `D`'s sign `sign` there, put into `out`, halved `depth` more times
    /// at most until its weights are positive: `false` where they never are.
    fn arc(&self, (from, to): (f64, f64), sign: f64, depth: u32, out: &mut Vec<Arc>) -> bool {
        let middle = f64::midpoint(from, to);
        let ((s0, c0), (sm, cm), (s1, c1)) = (sin_cos(from), sin_cos(middle), sin_cos(to));
        let circle = ([c0, cm, c1], [s0, sm, s1], [1.0, sin_cos(0.5 * (to - from)).1, 1.0]);
        let weights = homogeneous(&self.weight, circle).map(|w| w * sign);
        if out.len() >= MOST {
            return false;
        }
        if !weights.iter().all(|w| *w > 0.0 && w.is_finite()) {
            return depth > 0
                && self.arc((from, middle), sign, depth.saturating_sub(1), out)
                && self.arc((middle, to), sign, depth.saturating_sub(1), out);
        }
        let [x, y, z] = self.numerator.clone().map(|n| homogeneous(&n, circle).map(|c| c * sign));
        let points = std::array::from_fn(|k| {
            let at = |b: &[f64; 5]| b.get(k).copied().unwrap_or(f64::NAN);
            Vec3::new(at(&x), at(&y), at(&z)) * (1.0 / at(&weights))
        });
        out.push(Arc { to, weights, points });
        true
    }

    /// The rational B-spline of the curve from `lo` to `hi`, a stretch of a turn at most on which `D` does not vanish,
    /// in the world, its knots the angles where its arcs meet: `None` for a longer stretch, or where an arc's weights
    /// cannot be made positive.
    pub(super) fn spline(&self, pair: &Pair, (lo, hi): (f64, f64)) -> Option<SplineCurve> {
        let span = hi - lo;
        if span.is_nan() || span > 8.0 * WIDEST {
            return None;
        }
        let sign = if self.weight.at(f64::midpoint(lo, hi)) < 0.0 { -1.0 } else { 1.0 };
        let mut count = 1_u32;
        while span / f64::from(count) > WIDEST {
            count = count.saturating_add(1);
        }
        let mut arcs = Vec::new();
        for i in 0..count {
            let at = |k: u32| if k == count { hi } else { lo + span * f64::from(k) / f64::from(count) };
            if !self.arc((at(i), at(i.saturating_add(1))), sign, HALVINGS, &mut arcs) {
                return None;
            }
        }
        let (mut knots, mut poles, mut weights) = (vec![lo; 5], Vec::new(), Vec::new());
        for (i, arc) in arcs.iter().enumerate() {
            for k in usize::from(i != 0)..5 {
                poles.push(Point::from_vec3(pair.world(arc.points.get(k).copied().unwrap_or(arc.points[0]))));
                weights.push(arc.weights.get(k).copied().unwrap_or(f64::NAN));
            }
            knots.extend([arc.to; 4]);
        }
        knots.push(hi);
        SplineCurve::new(&BsplineCurve { degree: 4, knots, poles, weights: Some(weights) }).ok()
    }
}

/// The stretches of the turn between consecutive `cuts`, ascending from 0 to a whole turn, and from the last round to
/// the first, whose middles are `inside`, those that meet joined: the whole turn where there are no cuts and it is
/// inside, and none where it is not.
pub(super) fn stretches(cuts: &[f64], inside: impl Fn(f64) -> bool) -> Vec<(f64, f64)> {
    let (Some(first), Some(last)) = (cuts.first().copied(), cuts.last().copied()) else {
        return if inside(0.0) { vec![(0.0, TAU)] } else { Vec::new() };
    };
    let ends = cuts.windows(2).filter_map(|w| Some((*w.first()?, *w.last()?))).chain([(last, first + TAU)]);
    merged(ends.filter(|(lo, hi)| inside(f64::midpoint(*lo, *hi))).collect(), first)
}

/// `stretches`, each from one cut to the next round the turn starting at `first`, with those that meet joined into
/// one, the last with the first where the turn comes round.
fn merged(stretches: Vec<(f64, f64)>, first: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (lo, hi) in stretches {
        match out.last_mut() {
            Some(last) if last.1.to_bits() == lo.to_bits() => last.1 = hi,
            _ => out.push((lo, hi)),
        }
    }
    let wraps = out.len() > 1 && out.first().is_some_and(|f| f.0.to_bits() == first.to_bits());
    if wraps
        && let (Some(head), Some(tail)) = (out.first().copied(), out.last().copied())
        && tail.1.to_bits() == (head.0 + TAU).to_bits()
    {
        out.remove(0);
        if let Some(last) = out.last_mut() {
            last.1 = head.1 + TAU;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_8, PI};

    use occam_geom::Curve;

    use super::*;

    /// The circle `(cos θ, sin θ, 1) / D` for `D = weight`.
    fn circle(weight: &[f64]) -> Fan {
        let numerator = [Trig(vec![0.0, 1.0, 0.0]), Trig(vec![0.0, 0.0, 1.0]), Trig(vec![1.0])];
        Fan { numerator, weight: Trig(weight.to_vec()) }
    }

    #[test]
    fn an_arc_whose_weights_are_not_all_positive_is_halved_until_they_are() {
        // `1.01 − cos θ` is least at 0, and its weights on the arc either side of it dip below zero.
        let fan = circle(&[1.01, -1.0, 0.0]);
        let mut whole = Vec::new();
        assert!(!fan.arc((-FRAC_PI_8, FRAC_PI_8), 1.0, 0, &mut whole), "not halved, refused");
        assert!(fan.arc((-FRAC_PI_8, FRAC_PI_8), 1.0, HALVINGS, &mut whole));
        assert!(whole.len() > 1 && whole.iter().all(|a| a.weights.iter().all(|w| *w > 0.0)), "{}", whole.len());
        let pair = super::super::tests::unplaced(
            super::super::tests::ball(Vec3::new(0.0, 0.0, 0.0), 1.0),
            super::super::tests::upright(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 1.0),
        );
        let spline = fan.spline(&pair, (-FRAC_PI_8, FRAC_PI_8)).unwrap();
        for arc in &whole {
            let (got, want) = (spline.point(arc.to).to_vec3(), pair.world(fan.point(arc.to)));
            assert!((got - want).norm() <= 1e-13, "at {}: {got:?} against {want:?}", arc.to);
        }
        let flipped = Fan { numerator: fan.numerator.clone().map(|n| n.scaled(-1.0)), weight: fan.weight.scaled(-1.0) };
        let other = flipped.spline(&pair, (-FRAC_PI_8, FRAC_PI_8)).unwrap();
        assert!(
            (other.point(0.1).to_vec3() - spline.point(0.1).to_vec3()).norm() <= 1e-13,
            "a negative weight flipped"
        );
        assert!(circle(&[0.0, 1.0, 0.0]).spline(&pair, (1.0, 2.0)).is_none(), "a weight that changes sign at π/2");
        assert!(fan.spline(&pair, (1.0, 1.0 + 1.01 * TAU)).is_none(), "more than a turn");
        assert!(fan.spline(&pair, (1.0, f64::INFINITY)).is_none(), "no end");
        assert!(circle(&[1.0]).spline(&pair, (1.0, 1.0 + TAU)).is_some(), "a whole turn");
        let mut many = vec![Arc { to: 0.0, weights: [1.0; 5], points: [Vec3::new(0.0, 0.0, 0.0); 5] }; MOST - 1];
        assert!(circle(&[1.0]).arc((0.0, 0.1), 1.0, 0, &mut many), "one arc short of the most");
        assert!(!circle(&[1.0]).arc((0.0, 0.1), 1.0, 0, &mut many), "the most");
    }

    #[test]
    fn a_curve_is_within_a_ball_where_it_is_inside_its_sphere_round_the_turn() {
        let unit = circle(&[1.0]);
        let ball = |x: f64, radius: f64| Ball { centre: Vec3::new(x, 0.0, 1.0), radius };
        assert_eq!(unit.within(ball(0.0, 2.0)), vec![(0.0, TAU)], "all of it");
        assert_eq!(unit.within(ball(5.0, 1.0)), Vec::new(), "none of it");
        let [(lo, hi)] = unit.within(ball(1.0, 1.0))[..] else { panic!("{:?}", unit.within(ball(1.0, 1.0))) };
        assert!((lo - (TAU - PI / 3.0)).abs() <= 1e-12 && (hi - (TAU + PI / 3.0)).abs() <= 1e-12, "{lo} {hi}");
        assert!(ball(1.0, 1.0).holds(Vec3::new(1.5, 0.5, 1.5)) && !ball(1.0, 1.0).holds(Vec3::new(2.0, 0.5, 1.0)));
        assert!(!ball(1.0, 1.0).holds(Vec3::new(f64::NAN, 0.0, 1.0)), "nothing that is not finite");
        let pair = super::super::tests::unplaced(
            super::super::tests::ball(Vec3::new(2.0, 0.0, 0.0), 1.0),
            super::super::tests::upright(Vec3::new(0.0, 4.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 1.0),
        );
        let got = Ball::of(
            &pair,
            Bounds::around(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), occam_units::Length::from_millimetres(2.0)),
        );
        assert!((got.centre - Vec3::new(0.0, 0.0, 3.0)).norm() <= 1e-12, "about the box's middle, in the pair's frame");
        assert!(
            (got.radius - 1.01 * 2.0 * 3.0_f64.sqrt()).abs() <= 1e-12,
            "a hundredth past its corners: {}",
            got.radius
        );
    }

    #[test]
    fn stretches_that_meet_are_joined_the_last_with_the_first_round_the_turn() {
        assert_eq!(merged(vec![(1.0, 2.0), (2.0, 3.0), (4.0, 5.0)], 1.0), vec![(1.0, 3.0), (4.0, 5.0)]);
        assert_eq!(merged(vec![(1.0, 2.0), (4.0, 1.0 + TAU)], 1.0), vec![(4.0, 2.0 + TAU)], "round the turn");
        assert_eq!(merged(vec![(2.0, 3.0), (4.0, 1.0 + TAU)], 1.0), vec![(2.0, 3.0), (4.0, 1.0 + TAU)]);
        assert_eq!(merged(vec![(1.0, 2.0)], 1.0), vec![(1.0, 2.0)]);
        assert_eq!(merged(Vec::new(), 1.0), Vec::new());
    }
}
