//! Source: davis-rabinowitz-1984
//! A curve's length (PARITY 2.1): the integral of its speed, by Gauss-Legendre quadrature on intervals halved until the
//! two halves agree with their whole to the interval's share of the tolerance (Davis and Rabinowitz, sections 2.7 and
//! 6.2). The nodes and weights are computed here, as the roots of the Legendre polynomial found by Newton's method from
//! the usual cosine guesses, with the polynomial and its derivative from the three-term recurrence; no table is copied.

use occam_math::cos;
use occam_report::{Code, Fault, NamedValue, Quantity, Reason, Report};
use occam_units::Length;

use crate::curve::Curve;
use crate::support::positive;

/// How many points each interval's rule takes.
const POINTS: usize = 10;

/// The most intervals one length may be cut into before it gives up.
const BUDGET: usize = 1 << 16;

/// The Legendre polynomial of degree `POINTS` at `x`, and its derivative, by the recurrence
/// `(k + 1) P(k+1) = (2k + 1) x P(k) - k P(k-1)`.
fn legendre(x: f64) -> (f64, f64) {
    let (mut below, mut here) = (1.0, x);
    for k in 1..POINTS {
        let k = f64::from(u32::try_from(k).unwrap_or(0));
        let next = ((2.0 * k + 1.0) * x * here - k * below) / (k + 1.0);
        (below, here) = (here, next);
    }
    let n = f64::from(u32::try_from(POINTS).unwrap_or(0));
    (here, n * (x * here - below) / (x * x - 1.0))
}

/// The rule's nodes on `[-1, 1]` and their weights: the roots of the Legendre polynomial, each refined by Newton's
/// method from `cos(pi (i - 1/4) / (n + 1/2))` until its step no longer moves it, and `2 / ((1 - x^2) P'(x)^2)`.
pub(crate) fn rule() -> [(f64, f64); POINTS] {
    let n = f64::from(u32::try_from(POINTS).unwrap_or(0));
    std::array::from_fn(|i| {
        let i = f64::from(u32::try_from(i).unwrap_or(0)) + 1.0;
        let mut x = cos(std::f64::consts::PI * (i - 0.25) / (n + 0.5));
        for _ in 0..100 {
            let (p, dp) = legendre(x);
            let next = x - p / dp;
            if next.to_bits() == x.to_bits() {
                break;
            }
            x = next;
        }
        let (_, dp) = legendre(x);
        (x, 2.0 / ((1.0 - x * x) * dp * dp))
    })
}

/// The rule's estimate of the integral of `f` from `a` to `b`.
fn gauss(f: &dyn Fn(f64) -> f64, nodes: &[(f64, f64); POINTS], (a, b): (f64, f64)) -> f64 {
    let (middle, half) = (0.5 * a + 0.5 * b, 0.5 * (b - a));
    half * nodes.iter().map(|(x, w)| w * f(middle + half * x)).sum::<f64>()
}

/// The integral of `f` from `a` to `b`, `a` below `b`, and the sum of the differences between each accepted interval's
/// halves and its whole, which bounds its error in practice: each interval is halved until its halves agree with it to
/// its share of `tolerance` by width. That ends: an interval with no double inside has its middle at an end, so one half
/// is the whole of it and the other is empty, and they agree with it exactly.
///
/// # Errors
///
/// `measure.not-converged` where the budget of intervals is spent, or the integrand is not finite, first.
pub(crate) fn integral(f: &dyn Fn(f64) -> f64, (a, b): (f64, f64), tolerance: f64) -> Result<(f64, f64), Report> {
    let nodes = rule();
    let (mut total, mut error, mut spent) = (0.0, 0.0, 0_usize);
    let mut pending = vec![(a, b, gauss(f, &nodes, (a, b)))];
    while let Some((lo, hi, whole)) = pending.pop() {
        spent = spent.saturating_add(1);
        let middle = 0.5 * lo + 0.5 * hi;
        let (left, right) = (gauss(f, &nodes, (lo, middle)), gauss(f, &nodes, (middle, hi)));
        let difference = (left + right - whole).abs();
        let share = tolerance * (hi - lo) / (b - a);
        if !difference.is_finite() || spent > BUDGET {
            return Err(not_converged(error + difference, tolerance));
        }
        if difference <= share {
            (total, error) = (total + left + right, error + difference);
        } else {
            pending.extend([(lo, middle, left), (middle, hi, right)]);
        }
    }
    Ok((total, error))
}

/// The refusal of a measure that reached `reached` where `asked` was asked for.
pub(crate) fn not_converged(reached: f64, asked: f64) -> Report {
    Report::new(Code::NotConverged, Fault::NumericallySensitive)
        .with_reason(Reason::BudgetSpent)
        .with_value(NamedValue::new("reached", Quantity::Length, if reached.is_finite() { reached } else { f64::MAX }))
        .with_value(NamedValue::new("asked", Quantity::Length, asked))
}

/// The length of `c` from `lo` to `hi`, either way round, to within `tolerance`.
///
/// # Errors
///
/// `geom.size-not-positive` for a tolerance that is not a finite length above zero, `geom.undefined-parameter` for an
/// end outside the range or not finite, and `measure.not-converged` where the quadrature cannot reach the tolerance.
pub fn length(c: &dyn Curve, (lo, hi): (f64, f64), tolerance: Length) -> Result<Length, Report> {
    let tolerance = positive(tolerance)?;
    let range = c.range();
    range.check(lo)?;
    range.check(hi)?;
    let (a, b) = if hi < lo { (hi, lo) } else { (lo, hi) };
    let empty = a >= b;
    if empty {
        return Ok(Length::from_millimetres(0.0));
    }
    let speed = |t: f64| c.first(t).1.rate().norm();
    let (total, _) = integral(&speed, (a, b), tolerance)?;
    Ok(Length::from_millimetres(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_integrates_polynomials_of_degree_below_twice_its_points_exactly() {
        let nodes = rule();
        let weights: f64 = nodes.iter().map(|(_, w)| w).sum();
        assert!((weights - 2.0).abs() < 1e-14, "{weights}");
        for degree in [2_i32, 7, 12, 19] {
            let f = |x: f64| (0..degree).fold(1.0, |p, _| p * x);
            let want = if degree % 2 == 0 { 2.0 / f64::from(degree + 1) } else { 0.0 };
            assert!((gauss(&f, &nodes, (-1.0, 1.0)) - want).abs() < 1e-14, "degree {degree}");
        }
        assert!(nodes.iter().all(|(x, _)| x.abs() < 1.0) && nodes.windows(2).all(|p| p[1].0 < p[0].0));
    }

    #[test]
    fn an_integral_is_halved_until_it_agrees_to_the_tolerance() {
        // The integral of 1 / (1 + 25 x^2) over [-1, 1] is 2 atan(5) / 5, which one rule of ten points misses by 1e-3.
        let f = |x: f64| 1.0 / (1.0 + 25.0 * x * x);
        let want = 2.0 * occam_math::atan2(5.0, 1.0) / 5.0;
        assert!((gauss(&f, &rule(), (-1.0, 1.0)) - want).abs() > 1e-5);
        let (got, error) = integral(&f, (-1.0, 1.0), 1e-12).unwrap();
        assert!((got - want).abs() < 1e-12 && error < 1e-12, "{got} {want} {error}");
        assert!(error > 0.0, "the halves' differences are summed: {error}");
        let refused = integral(&|_| f64::NAN, (0.0, 1.0), 1e-9).unwrap_err();
        assert_eq!((refused.code, refused.reason), (Code::NotConverged, Some(Reason::BudgetSpent)));
    }

    #[test]
    fn each_interval_takes_its_share_of_the_tolerance_by_width() {
        // A peak at 3 on [0, 8]: the differences accepted sum to no more than the tolerance asked for.
        let f = |x: f64| 1.0 / (1.0 + 25.0 * (x - 3.0) * (x - 3.0));
        let want = (occam_math::atan2(25.0, 1.0) + occam_math::atan2(15.0, 1.0)) / 5.0;
        for tolerance in [1e-6, 1e-9] {
            let (got, error) = integral(&f, (0.0, 8.0), tolerance).unwrap();
            assert!(error <= tolerance && (got - want).abs() <= tolerance, "{tolerance}: {got} {want} {error}");
        }
    }

    #[test]
    fn an_interval_too_narrow_to_halve_is_taken_as_it_is() {
        // With no double inside, one half is the whole and the other is empty, so they agree with it exactly, even where
        // no share of the tolerance is left: a step is halved down to adjacent doubles and the rest is accepted whole.
        for x in [1.0 / 3.0, 0.7, 0.3, 2.0 / 3.0, 0.9, 0.6] {
            let step = |t: f64| if t < x { 0.0 } else { 1.0 };
            let (got, _) = integral(&step, (0.0, 1.0), 1e-12).unwrap();
            assert!((got - (1.0 - x)).abs() < 1e-12, "{x}: {got}");
            let (one, error) = integral(&step, (x, x.next_up()), 0.0).unwrap();
            assert!(error == 0.0 && (one - (x.next_up() - x)).abs() <= 8.0 * f64::EPSILON * one, "{x}: {one}");
        }
    }

    #[test]
    fn a_value_that_is_not_finite_stops_at_once_and_the_budget_is_spent_after_its_last_interval() {
        use std::cell::Cell;
        let calls = Cell::new(0_usize);
        // First, so that a mutant that never stops on a value that is not finite fails here rather than hang below.
        let poisoned = |_: f64| {
            calls.set(calls.get().saturating_add(1));
            f64::NAN
        };
        assert!(integral(&poisoned, (0.0, 1.0), 1e-9).is_err());
        assert_eq!(calls.get(), 3 * POINTS, "the whole and its two halves, once");
        calls.set(0);
        // A square wave of some 7e8 periods, a frequency that is no whole number, so that no symmetry of the rule makes an
        // interval's halves agree with it: no interval the budget can reach is smooth enough to be accepted.
        let stairs = |x: f64| {
            calls.set(calls.get().saturating_add(1));
            (x * 1_414_213_562.373_095).floor() % 2.0
        };
        let refused = integral(&stairs, (0.0, 1.0), 1e-300).unwrap_err();
        assert_eq!(calls.get(), POINTS * (1 + 2 * (BUDGET + 1)), "the first rule, then two halves of each interval");
        assert!(refused.value("reached").and_then(NamedValue::number).is_some_and(|r| r > 0.0), "{refused:?}");
    }

    use occam_math::sinh_cosh;
    use occam_units::{Direction, Point, Vec3};

    use crate::conformance::placement;
    use crate::{Circle, Ellipse, Hyperbola, Line, Parabola, SplineCurve};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn measured(c: &dyn Curve, range: (f64, f64)) -> f64 {
        length(c, range, mm(1e-11)).unwrap().millimetres()
    }

    /// An ellipse's perimeter by the Gauss-Kummer series, `pi (a + b) sum (1/2 choose n)^2 h^n`, `h = ((a - b) / (a + b))^2`.
    fn perimeter(a: f64, b: f64) -> f64 {
        let h = ((a - b) / (a + b)) * ((a - b) / (a + b));
        let (mut term, mut sum, mut power) = (1.0, 1.0, 1.0);
        for n in 1..80_i32 {
            term *= (1.5 - f64::from(n)) / f64::from(n);
            power *= h;
            sum += term * term * power;
        }
        std::f64::consts::PI * (a + b) * sum
    }

    /// `asinh(u)`, by Newton's method on the owned `sinh`.
    fn asinh(u: f64) -> f64 {
        (0..60).fold(u, |x, _| {
            let (s, c) = sinh_cosh(x);
            x - (s - u) / c
        })
    }

    /// The integral of `f` from `a` to `b` by Simpson's rule on `n` pieces, `n` even: an independent check.
    fn simpson(integrand: impl Fn(f64) -> f64, (from, to): (f64, f64), pieces: u32) -> f64 {
        let step = (to - from) / f64::from(pieces);
        let weight = |k: u32| if k % 2 == 1 { 4.0 } else { 2.0 };
        let inner: f64 = (1..pieces).map(|k| integrand(from + step * f64::from(k)) * weight(k)).sum();
        step / 3.0 * (integrand(from) + integrand(to) + inner)
    }

    #[test]
    fn known_lengths_for_every_analytic_curve_family() {
        let at = placement();
        let line =
            Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(1.0, -2.0, 0.5)));
        assert!((measured(&line, (-4.0, 7.5)) - 11.5).abs() < 1e-12);
        assert!((measured(&Circle::placed(&at, mm(10.0)).unwrap(), (0.5, 2.5)) - 20.0).abs() < 1e-11);
        let ellipse = Ellipse::placed(&at, mm(7.0), mm(3.0)).unwrap();
        let whole = measured(&ellipse, (0.0, std::f64::consts::TAU));
        assert!((whole - perimeter(7.0, 3.0)).abs() < 1e-10, "{whole} against {}", perimeter(7.0, 3.0));
        // The parabola t^2 / 4f, t: f (u sqrt(1 + u^2) + asinh u) at u = t / 2f.
        let f = 1.5;
        let primitive = |t: f64| {
            let u = t / (2.0 * f);
            f * (u * (1.0 + u * u).sqrt() + asinh(u))
        };
        let parabola = Parabola::placed(&at, mm(f)).unwrap();
        assert!((measured(&parabola, (-6.0, 4.0)) - (primitive(4.0) - primitive(-6.0))).abs() < 1e-10);
        let hyperbola = Hyperbola::placed(&at, mm(2.0), mm(5.0)).unwrap();
        let speed = |t: f64| {
            let (s, c) = sinh_cosh(t);
            (4.0 * s * s + 25.0 * c * c).sqrt()
        };
        let want = simpson(speed, (-1.5, 2.0), 200_000);
        assert!((measured(&hyperbola, (-1.5, 2.0)) - want).abs() < 1e-9, "{want}");
    }

    #[test]
    fn a_spline_measures_the_curve_it_is_not_its_parameter() {
        // A rational quarter circle of radius 3, whose parameter is not its angle, and a line with unequal poles.
        let w = 0.5f64.sqrt();
        let point = |x: f64, y: f64| Point::from_vec3(Vec3::new(x, y, 0.0));
        let quarter =
            SplineCurve::bezier(vec![point(3.0, 0.0), point(3.0, 3.0), point(0.0, 3.0)], Some(vec![1.0, w, 1.0]))
                .unwrap();
        assert!((measured(&quarter, (0.0, 1.0)) - 1.5 * std::f64::consts::PI).abs() < 1e-10);
        let straight = SplineCurve::bezier(vec![point(0.0, 0.0), point(0.5, 0.0), point(4.0, 0.0)], None).unwrap();
        assert!((measured(&straight, (0.0, 1.0)) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn a_length_either_way_round_is_the_same_and_nothing_is_measured_outside() {
        let circle = Circle::placed(&placement(), mm(2.0)).unwrap();
        assert_eq!(measured(&circle, (2.0, 1.0)), measured(&circle, (1.0, 2.0)));
        assert_eq!(measured(&circle, (1.0, 1.0)), 0.0);
        let arc = circle.trimmed(0.5, 1.0).unwrap();
        assert_eq!(length(&arc, (0.5, 2.0), mm(1e-9)).unwrap_err().code, Code::UndefinedParameter);
        assert_eq!(length(&circle, (0.0, 1.0), mm(0.0)).unwrap_err().code, Code::SizeNotPositive);
    }
}
