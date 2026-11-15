//! Source: davis-rabinowitz-1984
//! Adaptive Gauss-Legendre quadrature of several integrands at once, every evaluation shared: an interval is halved until
//! its halves agree with it, in every component, to the interval's share of that component's tolerance by width
//! (Davis and Rabinowitz, sections 2.7 and 6.2), on occam-geom's ten-point rule, as a curve's length is. Each interval
//! halved spends one of a budget that everything one measure integrates shares, so that no integrand, however it is
//! drawn, can make the measure run without end.

use core::cell::Cell;
use std::sync::OnceLock;

use occam_geom::gauss_legendre;

/// The rule's nodes and weights, found once: an integral at each point of another takes them thousands of times.
fn nodes() -> &'static [(f64, f64)] {
    static RULE: OnceLock<[(f64, f64); 10]> = OnceLock::new();
    RULE.get_or_init(gauss_legendre)
}

/// How many intervals the integrals of one measure may halve in all.
pub(super) struct Budget {
    left: Cell<u64>,
}

impl Budget {
    /// A budget of `intervals`.
    pub(super) const fn new(intervals: u64) -> Self {
        Self { left: Cell::new(intervals) }
    }

    /// Spends one interval: false, spending nothing, where none is left.
    pub(super) fn spend(&self) -> bool {
        let left = self.left.get();
        self.left.set(left.saturating_sub(1));
        left > 0
    }
}

/// The integrals of each component, and the sum of the differences between each accepted interval's halves and its
/// whole, which bounds each one's error in practice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Integral<const N: usize> {
    pub value: [f64; N],
    pub error: [f64; N],
}

impl<const N: usize> Integral<N> {
    /// Nothing yet.
    pub(super) const ZERO: Self = Self { value: [0.0; N], error: [0.0; N] };

    /// This and `other` together.
    pub(super) fn plus(self, other: Self, sign: f64) -> Self {
        let mut sum = self;
        for ((v, e), (w, f)) in sum.value.iter_mut().zip(&mut sum.error).zip(other.value.iter().zip(&other.error)) {
            *v += sign * w;
            *e += f;
        }
        sum
    }
}

/// `a` and `b` added component by component.
fn added<const N: usize>(a: [f64; N], b: [f64; N]) -> [f64; N] {
    let mut sum = a;
    sum.iter_mut().zip(b).for_each(|(s, x)| *s += x);
    sum
}

/// The rule's estimate of each component's integral of `f` from `lo` to `hi`.
fn rule<const N: usize>(f: &dyn Fn(f64) -> [f64; N], nodes: &[(f64, f64)], (lo, hi): (f64, f64)) -> [f64; N] {
    let (middle, half) = (0.5 * lo + 0.5 * hi, 0.5 * (hi - lo));
    let mut sum = [0.0; N];
    for &(x, w) in nodes {
        sum.iter_mut().zip(f(middle + half * x)).for_each(|(s, y)| *s += w * y);
    }
    sum.map(|s| half * s)
}

/// The integrals of `f` from `a` to `b`, either way round, each component to its `tolerance`, which may be infinite
/// where a component is carried along and never asked for. Each interval is halved until its halves agree with it to
/// its share of every tolerance by width; an interval with no double inside ends that, as its middle is an end, so
/// one half is the whole of it and the other is empty.
///
/// # Errors
///
/// The error reached so far, with the last interval's differences, where the budget is spent or a value is not finite
/// first.
pub(super) fn integral<const N: usize>(
    f: &dyn Fn(f64) -> [f64; N],
    (a, b): (f64, f64),
    tolerance: [f64; N],
    budget: &Budget,
) -> Result<Integral<N>, [f64; N]> {
    let nodes = nodes();
    let mut done = Integral::ZERO;
    if a.to_bits() == b.to_bits() {
        return Ok(done);
    }
    let mut pending = vec![(a, b, rule(f, nodes, (a, b)))];
    while let Some((lo, hi, whole)) = pending.pop() {
        let middle = 0.5 * lo + 0.5 * hi;
        let halves = (rule(f, nodes, (lo, middle)), rule(f, nodes, (middle, hi)));
        let both = added(halves.0, halves.1);
        let mut differences = both;
        differences.iter_mut().zip(whole).for_each(|(d, w)| *d = (*d - w).abs());
        if !differences.iter().all(|d| d.is_finite()) || !budget.spend() {
            return Err(added(done.error, differences));
        }
        let width = (hi - lo) / (b - a);
        if differences.iter().zip(tolerance).all(|(d, t)| *d <= t * width) {
            done = done.plus(Integral { value: both, error: differences }, 1.0);
        } else {
            pending.push((middle, hi, halves.1));
            pending.push((lo, middle, halves.0));
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    const ENOUGH: u64 = 1 << 20;

    #[test]
    fn components_are_integrated_together_each_to_its_own_tolerance() {
        // 1 / (1 + 25 x^2) over [-1, 1] is 2 atan(5) / 5, which one rule misses by about 1e-3, and x^2 is 2 / 3 exactly.
        let f = |x: f64| [1.0 / (1.0 + 25.0 * x * x), x * x, 1.0];
        let want = 2.0 * occam_math::atan2(5.0, 1.0) / 5.0;
        let got = integral(&f, (-1.0, 1.0), [1e-12, 1e-12, f64::INFINITY], &Budget::new(ENOUGH)).unwrap();
        assert!((got.value[0] - want).abs() < 1e-12 && got.error[0] < 1e-12 && got.error[0] > 0.0, "{got:?}");
        assert!((got.value[1] - 2.0 / 3.0).abs() < 1e-15 && (got.value[2] - 2.0).abs() < 1e-15, "{got:?}");
        // The other way round, the integrals change sign.
        let back = integral(&f, (1.0, -1.0), [1e-12, 1e-12, f64::INFINITY], &Budget::new(ENOUGH)).unwrap();
        assert!((back.value[0] + want).abs() < 1e-12 && (back.value[2] + 2.0).abs() < 1e-15, "{back:?}");
        // Asked to only a coarse tolerance, the first is left further off, with fewer intervals spent.
        let budget = Budget::new(ENOUGH);
        let coarse = integral(&f, (-1.0, 1.0), [1e-3, 1.0, f64::INFINITY], &budget).unwrap();
        assert!((coarse.value[0] - want).abs() > 1e-9 && (coarse.value[0] - want).abs() < 1e-3, "{coarse:?}");
        assert!(budget.left.get() > ENOUGH - 8, "{}", budget.left.get());
    }

    #[test]
    fn each_interval_takes_its_share_of_the_tolerance_by_width() {
        // A peak at 3 on [0, 8]: the differences accepted sum to no more than the tolerance, and the value is as close.
        let f = |x: f64| [1.0 / (1.0 + 25.0 * (x - 3.0) * (x - 3.0))];
        let want = (occam_math::atan2(25.0, 1.0) + occam_math::atan2(15.0, 1.0)) / 5.0;
        for tolerance in [1e-6, 1e-9] {
            let got = integral(&f, (0.0, 8.0), [tolerance], &Budget::new(ENOUGH)).unwrap();
            assert!(got.error[0] <= tolerance && (got.value[0] - want).abs() <= tolerance, "{tolerance}: {got:?}");
        }
    }

    #[test]
    fn an_empty_range_is_nothing_and_needs_no_evaluation() {
        let calls = Cell::new(0_u32);
        let f = |_: f64| {
            calls.set(calls.get() + 1);
            [1.0]
        };
        assert_eq!(integral(&f, (2.5, 2.5), [0.0], &Budget::new(0)), Ok(Integral::ZERO));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn a_value_that_is_not_finite_or_a_spent_budget_stops_it_with_the_error_reached() {
        let calls = Cell::new(0_usize);
        let poisoned = |_: f64| {
            calls.set(calls.get() + 1);
            [f64::NAN, 1.0]
        };
        let error = integral(&poisoned, (0.0, 1.0), [1.0, 1.0], &Budget::new(ENOUGH)).unwrap_err();
        assert!(error[0].is_nan() && error[1].abs() < 1e-15, "{error:?}");
        assert_eq!(calls.get(), 30, "the whole and its two halves, once");
        // A step at a third: each interval holding it is halved, each one spending the budget, until none is left.
        let step = |x: f64| [if x < 1.0 / 3.0 { 0.0 } else { 1.0 }];
        let budget = Budget::new(4);
        let reached = integral(&step, (0.0, 1.0), [1e-12], &budget).unwrap_err();
        assert!(reached[0] > 1e-12, "{reached:?}");
        assert!(!budget.spend(), "all spent");
        // With just enough, the step is found to the tolerance: the halves of an interval without a double inside agree.
        let got = integral(&step, (0.0, 1.0), [1e-12], &Budget::new(ENOUGH)).unwrap();
        assert!((got.value[0] - 2.0 / 3.0).abs() < 1e-12, "{got:?}");
    }

    #[test]
    fn a_budget_spends_down_to_nothing_and_stays_there() {
        let budget = Budget::new(2);
        assert!(budget.spend() && budget.spend());
        assert!(!budget.spend() && !budget.spend());
        assert_eq!(budget.left.get(), 0);
    }

    #[test]
    fn integrals_add_and_take_away_their_values_and_always_add_their_errors() {
        let a = Integral { value: [1.0, 2.0], error: [0.5, 0.25] };
        let b = Integral { value: [4.0, 8.0], error: [1.0, 2.0] };
        assert_eq!(a.plus(b, 1.0), Integral { value: [5.0, 10.0], error: [1.5, 2.25] });
        assert_eq!(a.plus(b, -1.0), Integral { value: [-3.0, -6.0], error: [1.5, 2.25] });
    }
}
